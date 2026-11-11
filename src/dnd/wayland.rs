//! A `wl_data_device` per seat on winit's own Wayland connection, with its own event queue and
//! thread (the way smithay-clipboard reads the clipboard). Only `text/uri-list` drops are taken.
//! A protocol error here would kill winit's connection too, so requests follow the spec closely.

use super::{imp::with, uri};
use eframe::egui;
use std::ffi::c_void;
use std::io::Read;
use std::os::fd::AsFd;
use std::path::PathBuf;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use wayland_backend::client::Backend;
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_data_device::{self, WlDataDevice};
use wayland_client::protocol::wl_data_device_manager::{DndAction, WlDataDeviceManager};
use wayland_client::protocol::wl_data_offer::{self, WlDataOffer};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum, event_created_child};

const URI_LIST: &str = "text/uri-list";

pub(super) fn start(display: NonNull<c_void>, ctx: egui::Context) -> Result<(), String> {
    // SAFETY: winit's wl_display, alive for as long as the event loop, i.e. the whole app.
    let backend = unsafe { Backend::from_foreign_display(display.as_ptr().cast()) };
    let conn = Connection::from_backend(backend);
    let (globals, mut queue) = registry_queue_init::<State>(&conn).map_err(|e| e.to_string())?;
    let qh = queue.handle();
    // v3 adds drag actions and `finish`, which file managers wait for.
    let manager: WlDataDeviceManager = globals.bind(&qh, 1..=3, ()).map_err(|e| e.to_string())?;
    let seats: Vec<u32> = globals.contents().with_list(|list| {
        list.iter()
            .filter(|g| g.interface == WlSeat::interface().name)
            .map(|g| g.name)
            .collect()
    });
    for name in seats {
        let seat: WlSeat = globals.registry().bind(name, 1, &qh, ());
        manager.get_data_device(&seat, &qh, ());
    }
    let mut state = State {
        conn,
        ctx,
        offer: None,
    };
    std::thread::Builder::new()
        .name("wayland-drop".into())
        .spawn(move || while queue.blocking_dispatch(&mut state).is_ok() {})
        .map_err(|e| e.to_string())?;
    Ok(())
}

struct State {
    conn: Connection,
    ctx: egui::Context,
    /// The drag over the window, if it carries files.
    offer: Option<WlDataOffer>,
}

impl State {
    fn hover(&self, on: bool) {
        with(|d| d.hovering = on);
        crate::wake(&self.ctx);
    }

    fn leave(&mut self) {
        if let Some(offer) = self.offer.take() {
            offer.destroy();
            self.hover(false);
        }
    }

    /// Read the dropped URI list and hand the files to the app.
    fn drop_files(&mut self) {
        let Some(offer) = self.offer.take() else {
            return;
        };
        let paths = self.receive(&offer);
        let data = offer.data::<Offer>();
        // `finish` without an accepted action is a protocol error.
        if offer.version() >= 3 && data.is_some_and(|d| d.action.load(Relaxed) != 0) {
            offer.finish();
        }
        offer.destroy();
        with(|d| {
            d.hovering = false;
            d.dropped.extend(paths);
        });
        crate::wake(&self.ctx);
    }

    fn receive(&self, offer: &WlDataOffer) -> Vec<PathBuf> {
        let Ok((mut reader, writer)) = std::io::pipe() else {
            return Vec::new();
        };
        offer.receive(URI_LIST.into(), writer.as_fd());
        drop(writer);
        if self.conn.flush().is_err() {
            return Vec::new();
        }
        // shortcut: blocks this thread (not the UI) until the source closes the pipe; a source
        // that never does stops later drops. Add a timeout if that shows up.
        let mut text = String::new();
        let _ = reader.read_to_string(&mut text);
        uri::paths(&text)
    }
}

/// What an offer said about itself: whether it has files, and the action the compositor chose.
#[derive(Default)]
struct Offer {
    files: AtomicBool,
    action: AtomicU32,
}

impl Dispatch<WlDataDevice, ()> for State {
    fn event(
        state: &mut Self,
        _: &WlDataDevice,
        event: wl_data_device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_device::Event::Enter { serial, id, .. } => {
                state.leave();
                let Some(offer) = id else { return };
                if !offer.data::<Offer>().is_some_and(|d| d.files.load(Relaxed)) {
                    offer.accept(serial, None);
                    offer.destroy();
                    return;
                }
                offer.accept(serial, Some(URI_LIST.into()));
                if offer.version() >= 3 {
                    offer.set_actions(DndAction::Copy, DndAction::Copy);
                }
                state.offer = Some(offer);
                state.hover(true);
            }
            wl_data_device::Event::Leave => state.leave(),
            wl_data_device::Event::Drop => state.drop_files(),
            // Clipboard offers: egui-winit reads the clipboard on its own.
            wl_data_device::Event::Selection { id: Some(offer) } => offer.destroy(),
            _ => {}
        }
    }

    event_created_child!(State, WlDataDevice, [
        wl_data_device::EVT_DATA_OFFER_OPCODE => (WlDataOffer, Offer::default())
    ]);
}

impl Dispatch<WlDataOffer, Offer> for State {
    fn event(
        _: &mut Self,
        _: &WlDataOffer,
        event: wl_data_offer::Event,
        data: &Offer,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_offer::Event::Offer { mime_type } if mime_type == URI_LIST => {
                data.files.store(true, Relaxed);
            }
            wl_data_offer::Event::Action {
                dnd_action: WEnum::Value(a),
            } => data.action.store(a.bits(), Relaxed),
            _ => {}
        }
    }
}

/// Objects whose events don't matter here.
macro_rules! ignore_events {
    ($($proxy:ty, $data:ty);*) => {$(
        impl Dispatch<$proxy, $data> for State {
            fn event(
                _: &mut Self,
                _: &$proxy,
                _: <$proxy as Proxy>::Event,
                _: &$data,
                _: &Connection,
                _: &QueueHandle<Self>,
            ) {
            }
        }
    )*};
}

ignore_events!(WlRegistry, GlobalListContents; WlDataDeviceManager, (); WlSeat, ());
