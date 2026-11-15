//! macOS: videos Finder hands to Chyk. Open With sends an Open Documents Apple event (not argv),
//! and the "Edit with Chyk" Quick Action (`NSServices` in Info.plist) calls the services provider.
//! Both queue the paths; the app takes them each frame like a drop (`App::shortcuts`).

use eframe::egui;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock, PoisonError};

static PENDING: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());
static CTX: OnceLock<egui::Context> = OnceLock::new();

fn push(paths: Vec<PathBuf>) {
    if paths.is_empty() {
        return;
    }
    PENDING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .extend(paths);
    if let Some(ctx) = CTX.get() {
        crate::wake(ctx);
    }
}

/// The videos that arrived since the last call.
pub fn take() -> Vec<PathBuf> {
    std::mem::take(&mut *PENDING.lock().unwrap_or_else(PoisonError::into_inner))
}

/// Lets later arrivals wake the window. Files that came during launch are already queued.
pub fn set_context(ctx: &egui::Context) {
    let _ = CTX.set(ctx.clone());
}

pub use imp::install;

mod imp {
    use super::push;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObject};
    use objc2::{ClassType, class, define_class, msg_send, sel};
    use objc2_foundation::NSString;
    use std::path::PathBuf;

    /// Apple event codes: kCoreEventClass, kAEOpenDocuments, keyDirectObject.
    const CORE: u32 = u32::from_be_bytes(*b"aevt");
    const OPEN_DOCS: u32 = u32::from_be_bytes(*b"odoc");
    const DIRECT: u32 = u32::from_be_bytes(*b"----");

    unsafe extern "C" {
        /// AppKit: re-read the Services this app offers.
        fn NSUpdateDynamicServices();
    }

    /// An `NSURL`'s path.
    fn path_of(url: Option<Retained<AnyObject>>) -> Option<PathBuf> {
        let path: Option<Retained<NSString>> = unsafe { msg_send![&url?, path] };
        path.map(|p| PathBuf::from(p.to_string()))
    }

    define_class!(
        #[unsafe(super(NSObject))]
        #[name = "ChykOpenHandler"]
        struct Handler;

        impl Handler {
            /// AppKit installs its own Open Documents handler while launching; this runs after.
            #[unsafe(method(willFinishLaunching:))]
            fn will_finish(&self, _note: &AnyObject) {
                unsafe {
                    let events: Retained<AnyObject> =
                        msg_send![class!(NSAppleEventManager), sharedAppleEventManager];
                    let _: () = msg_send![
                        &events,
                        setEventHandler: self,
                        andSelector: sel!(handleOpen:withReply:),
                        forEventClass: CORE,
                        andEventID: OPEN_DOCS
                    ];
                    let app: Retained<AnyObject> =
                        msg_send![class!(NSApplication), sharedApplication];
                    let _: () = msg_send![&app, setServicesProvider: self];
                    NSUpdateDynamicServices();
                }
            }

            /// Open With, or a video dropped on the Dock icon: a list of file URLs.
            #[unsafe(method(handleOpen:withReply:))]
            fn handle_open(&self, event: &AnyObject, _reply: &AnyObject) {
                let paths = unsafe {
                    let list: Option<Retained<AnyObject>> =
                        msg_send![event, paramDescriptorForKeyword: DIRECT];
                    let Some(list) = list else { return };
                    let n: isize = msg_send![&list, numberOfItems];
                    let items: Vec<Retained<AnyObject>> = if n == 0 {
                        vec![list]
                    } else {
                        (1..=n)
                            .filter_map(|i| msg_send![&list, descriptorAtIndex: i])
                            .collect()
                    };
                    items
                        .iter()
                        .filter_map(|d| {
                            let url: Option<Retained<AnyObject>> = msg_send![d, fileURLValue];
                            path_of(url)
                        })
                        .collect()
                };
                push(paths);
            }

            /// The "Edit with Chyk" Quick Action: file URLs on the pasteboard.
            #[unsafe(method(editWithChyk:userData:error:))]
            fn edit_with_chyk(
                &self,
                pboard: &AnyObject,
                _data: Option<&AnyObject>,
                _error: *mut *mut AnyObject,
            ) {
                let paths = unsafe {
                    let items: Option<Retained<AnyObject>> = msg_send![pboard, pasteboardItems];
                    let Some(items) = items else { return };
                    let n: usize = msg_send![&items, count];
                    let kind = NSString::from_str("public.file-url");
                    (0..n)
                        .filter_map(|i| {
                            let item: Retained<AnyObject> = msg_send![&items, objectAtIndex: i];
                            let s: Option<Retained<NSString>> =
                                msg_send![&item, stringForType: &*kind];
                            let url: Option<Retained<AnyObject>> =
                                msg_send![class!(NSURL), URLWithString: &*s?];
                            path_of(url)
                        })
                        .collect()
                };
                push(paths);
            }
        }
    );

    /// Before eframe starts: wait for AppKit's will-finish-launching, then take over Open
    /// Documents and the Quick Action. Any earlier and AppKit would replace the handler.
    pub fn install() {
        unsafe {
            let handler: Retained<Handler> = msg_send![Handler::class(), new];
            let center: Retained<AnyObject> =
                msg_send![class!(NSNotificationCenter), defaultCenter];
            let name = NSString::from_str("NSApplicationWillFinishLaunchingNotification");
            let _: () = msg_send![
                &center,
                addObserver: &*handler,
                selector: sel!(willFinishLaunching:),
                name: &*name,
                object: None::<&AnyObject>
            ];
            // Neither the notification center nor the event manager keeps it: it lives for good.
            std::mem::forget(handler);
        }
    }
}
