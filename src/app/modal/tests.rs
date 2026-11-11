use super::*;

#[test]
fn unexported_edits_enter_exports_and_cmd_backspace_discards() {
    for c in [Confirm::Quit, Confirm::CloseVideo] {
        assert_eq!(modal_key(&c, true, false), Some(Choice::Export));
        assert_eq!(modal_key(&c, false, true), Some(Choice::Discard));
        assert_eq!(modal_key(&c, false, false), None);
    }
}

#[test]
fn another_file_enter_opens_a_window_and_cmd_backspace_replaces() {
    let c = Confirm::Open(PathBuf::from("a.mp4"));
    assert_eq!(modal_key(&c, true, false), Some(Choice::NewWindow));
    assert_eq!(modal_key(&c, false, true), Some(Choice::Replace));
}

#[test]
fn focused_button_keeps_enter() {
    assert!(enter_is_primary(false));
    assert!(!enter_is_primary(true));
}

#[test]
fn tab_stays_inside_a_modal_layer() {
    let ctx = egui::Context::default();
    let tab = |pressed| egui::RawInput {
        events: vec![egui::Event::Key {
            key: Key::Tab,
            physical_key: None,
            pressed,
            repeat: false,
            modifiers: Modifiers::NONE,
        }],
        ..Default::default()
    };
    let (mut behind, mut inside) = (None, None);
    for i in 0..8 {
        let input = if i % 2 == 0 { tab(true) } else { tab(false) };
        let mut full = ctx.run_ui(input, |ui| {
            behind = Some(
                ui.interact(ui.max_rect(), egui::Id::new("behind"), Sense::click())
                    .id,
            );
            egui::Modal::new("m".into()).show(ui.ctx(), |ui| {
                inside = Some(
                    ui.interact(
                        Rect::from_min_size(pos2(5.0, 5.0), vec2(20.0, 20.0)),
                        egui::Id::new("inside"),
                        Sense::click(),
                    )
                    .id,
                );
            });
        });
        full.textures_delta.clear();
        if i > 1 {
            let focused = ctx.memory(|m| m.focused());
            assert_ne!(focused, behind);
        }
    }
    assert_eq!(ctx.memory(|m| m.focused()), inside);
}

#[test]
fn keyboard_open_schedules_the_frame_that_shows_it() {
    let ctx = egui::Context::default();
    let id = egui::Id::new("p");
    let frame = || {
        let mut full = ctx.run_ui(Default::default(), |ui| {
            repaint_on_open(ui.ctx(), id);
            egui::Modal::new(id).show(ui.ctx(), |ui| {
                ui.label("hello");
            });
        });
        full.textures_delta.clear();
        full.viewport_output.values().next().unwrap().repaint_delay
    };
    // The first frame only sizes the area; it must ask for the next one with no input.
    assert!(frame() <= Duration::from_millis(50));
    assert!(ctx.memory(|m| m.area_rect(id)).is_some());
    frame();
    assert!(
        ctx.memory(|m| m.area_rect(id))
            .is_some_and(|r| r.area() > 0.0)
    );
}
