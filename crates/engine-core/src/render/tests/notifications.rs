use super::super::*;
use deflorta_common::notification::{NotificationOptions, NotificationState};

#[test]
#[ignore = "Requires a native GPU adapter"]
fn notification_outer_corners_render_on_scaled_and_narrow_surfaces() {
    let files = crate::GameFiles::open(&crate::workspace_dir().join("game")).unwrap();
    let mut assets = Assets::new(files.clone());
    let captures = std::env::var_os("DEFLORTA_TEST_CAPTURE_DIR").map(std::path::PathBuf::from);
    if let Some(directory) = &captures {
        std::fs::create_dir_all(directory).unwrap();
    }
    for (width, height) in [(320_u32, 180_u32), (720, 1280), (2048, 923)] {
        let mut renderer = pollster::block_on(Renderer::offscreen(width, height)).unwrap();
        for count in [1_u64, 2, 3, 5] {
            let mut ui = Ui::new(deflorta_ui::ui::text::TextSystem::new(&files));
            ui.set_config(1280.0, 720.0, "Noto Sans");
            ui.set_surface_size(width.as_(), height.as_());
            let now = Instant::now();
            for id in 1..=count {
                let (message, state) = match id {
                    1 => ("Enabling self-voicing…", NotificationState::Loading),
                    2 => ("Saved", NotificationState::Info),
                    3 => ("Download complete", NotificationState::Success),
                    _ => ("Preferences updated", NotificationState::Info),
                };
                ui.show_notification(
                    id,
                    message.into(),
                    NotificationOptions {
                        state,
                        duration: None,
                    },
                    now,
                );
            }
            let items = ui.draw(&mut assets, now);
            let panels: Vec<_> = items
                .iter()
                .filter_map(|item| match item {
                    DrawItem::Quad(panel) if panel.border_width > 0.0 => Some(panel.clone()),
                    _ => None,
                })
                .collect();
            let image = renderer
                .render_to_image(items, &mut ui, &mut assets, Color([0.02, 0.02, 0.04, 1.0]))
                .unwrap();
            for panel in panels {
                let left = (panel.rect.x + 2.0).round();
                let right = (panel.rect.x + panel.rect.w - 3.0).round();
                let top = (panel.rect.y + 2.0).round();
                let bottom = (panel.rect.y + panel.rect.h - 3.0).round();
                for ((x, y), radius) in [(left, top), (right, top), (right, bottom), (left, bottom)]
                    .into_iter()
                    .zip(panel.radii)
                {
                    let inside = image.get_pixel(x.as_(), y.as_())[0] > 15;
                    assert_eq!(
                        inside,
                        radius <= 0.0,
                        "Corner at {x},{y} with radius {radius} on {width}x{height}"
                    );
                }
            }
            if let Some(directory) = &captures {
                image
                    .save(directory.join(format!("notifications-{count}-{width}x{height}.png")))
                    .unwrap();
            }
        }
    }
}
