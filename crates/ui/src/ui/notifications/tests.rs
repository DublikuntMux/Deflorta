use super::*;
use crate::assets::Assets;
use crate::ui::desc::NodeDesc;
use crate::ui::text::TextSystem;
use std::collections::HashMap;
use std::time::Duration;

fn ui() -> (Ui, Assets) {
    let files = crate::GameFiles::open(&crate::workspace_dir().join("game")).unwrap();
    let mut ui = Ui::new(TextSystem::new(&files));
    ui.set_config(1280.0, 720.0, "Noto Sans");
    (ui, Assets::new(files))
}

#[test]
fn loading_pill_is_top_centered_with_compact_bounds_and_correct_line_height() {
    let (mut ui, mut assets) = ui();
    let now = Instant::now();
    ui.show_notification(
        1,
        "Enabling self-voicing…".into(),
        NotificationOptions {
            state: NotificationState::Loading,
            duration: None,
        },
        now,
    );
    for (width, height) in [
        (1280.0, 720.0),
        (2048.0, 923.0),
        (320.0, 180.0),
        (720.0, 1280.0),
    ] {
        ui.set_surface_size(width, height);
        let items = ui.draw(&mut assets, now);
        let DrawItem::Quad(panel) = &items[0] else {
            panic!("Missing pill")
        };
        let scale = ui.scale.clamp(0.75, 2.0);
        assert!((panel.rect.center().0 - width / 2.0).abs() < 0.001);
        assert!(f32::mul_add(-16.0, scale, panel.rect.y).abs() < 0.001);
        assert!(f32::mul_add(-52.0, scale, panel.rect.h).abs() < 0.001);
        assert_eq!(panel.radii, [panel.rect.h / 2.0; 4]);
        assert!(panel.rect.x >= 0.0 && panel.rect.x + panel.rect.w <= width);
        assert!(panel.rect.y + panel.rect.h <= height);
        let DrawItem::Text(text) = items.last().unwrap() else {
            panic!("Missing message")
        };
        let entry = ui.text.entry(&text.id).unwrap();
        assert_eq!(text.scale, 1.0);
        let runs: Vec<_> = entry.buffer.layout_runs().collect();
        assert_eq!(runs.len(), 1);
        assert!(f32::mul_add(-25.2, scale, runs[0].line_height).abs() < 0.001);
        assert!(text.clip.h >= runs[0].line_height);
        assert!(runs[0].line_w * text.scale <= text.clip.w);
        assert!(runs[0].line_height * text.scale <= text.clip.h);
    }
    let first = ui.draw(&mut assets, now);
    let later = ui.draw(&mut assets, now + Duration::from_millis(250));
    let (DrawItem::Quad(first), DrawItem::Quad(later)) = (&first[1], &later[1]) else {
        panic!("Missing spinner")
    };
    assert_ne!(first.rect, later.rect);
}

#[test]
fn connected_stacks_round_only_their_outer_corners() {
    let (mut ui, mut assets) = ui();
    let now = Instant::now();
    for count in [1_u64, 2, 3, 5] {
        for id in 1..=5 {
            ui.dismiss_notification(id);
        }
        for id in 1..=count {
            ui.show_notification(
                id,
                format!("Notice {id}"),
                NotificationOptions::default(),
                now,
            );
        }
        let panels: Vec<_> = ui
            .draw(&mut assets, now)
            .into_iter()
            .filter_map(|item| {
                if let DrawItem::Quad(panel) = item {
                    Some(panel)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(panels.len(), usize::try_from(count).unwrap());
        for (position, panel) in panels.iter().enumerate() {
            let top = if position == 0 {
                panel.rect.h / 2.0
            } else {
                0.0
            };
            let bottom = if position + 1 == panels.len() {
                panel.rect.h / 2.0
            } else {
                0.0
            };
            assert_eq!(panel.radii, [top, top, bottom, bottom]);
        }
        for pair in panels.windows(2) {
            assert_eq!(pair[0].rect.x, pair[1].rect.x);
            assert_eq!(pair[0].rect.w, pair[1].rect.w);
            assert!((pair[1].rect.y - pair[0].rect.y - pair[0].rect.h).abs() < 0.001);
        }
    }
}

#[test]
fn narrow_notifications_wrap_without_scaling_or_clipping_their_text() {
    let (mut ui, mut assets) = ui();
    let now = Instant::now();
    for message in ["Enabling self-voicing…", "self_voicing_initialization"] {
        ui.show_notification(
            1,
            message.into(),
            NotificationOptions {
                state: NotificationState::Loading,
                duration: None,
            },
            now,
        );
        for (width, height) in [
            (160.0, 320.0),
            (240.0, 480.0),
            (320.0, 180.0),
            (720.0, 1280.0),
            (2048.0, 923.0),
        ] {
            ui.set_surface_size(width, height);
            let items = ui.draw(&mut assets, now);
            let DrawItem::Text(text) = items.last().unwrap() else {
                panic!("Missing message")
            };
            let runs: Vec<_> = ui
                .text
                .entry(&text.id)
                .unwrap()
                .buffer
                .layout_runs()
                .collect();
            assert_eq!(text.scale, 1.0);
            assert!(!runs.is_empty() && runs.len() <= 3);
            assert!(text.y >= text.clip.y);
            for run in &runs {
                assert!(run.line_w <= text.clip.w + 0.01);
                assert!(
                    text.y + run.line_top + run.line_height <= text.clip.y + text.clip.h + 0.01
                );
            }
            assert_eq!(
                runs.iter()
                    .flat_map(|run| run.glyphs)
                    .map(|glyph| glyph.end)
                    .max(),
                Some(message.len())
            );
        }
    }
}

#[test]
fn handles_keep_identity_across_updates_commits_expiration_and_dismissal() {
    let (mut ui, mut assets) = ui();
    let now = Instant::now();
    ui.show_notification(
        1,
        "Working".into(),
        NotificationOptions {
            state: NotificationState::Loading,
            duration: None,
        },
        now,
    );
    ui.show_notification(2, "Saved".into(), NotificationOptions::default(), now);
    assert_eq!(
        ui.next_notification_deadline(),
        Some(now + Duration::from_secs(2))
    );
    ui.draw(&mut assets, now);
    let tree = ui.accessibility_update("Test");
    let (working_id, working) = tree
        .nodes
        .iter()
        .find(|(_, n)| n.label() == Some("Working"))
        .unwrap();
    assert_eq!(working.live(), Some(accesskit::Live::Polite));
    assert!(working.bounds().is_some());
    assert!(
        ui.speech_snapshot()
            .content
            .iter()
            .any(|(_, message)| message == "Working")
    );
    ui.commit(NodeDesc::default(), true, &HashMap::new(), &assets, now);
    assert!(ui.text.entry("@notification:1").is_some());

    ui.update_notification(
        1,
        "Ready".into(),
        NotificationOptions {
            state: NotificationState::Success,
            duration: Some(3.0),
        },
        now + Duration::from_secs(1),
    );
    ui.draw(&mut assets, now + Duration::from_secs(1));
    assert!(!ui.is_animating(now));
    assert!(
        ui.accessibility_update("Test")
            .nodes
            .iter()
            .any(|(id, n)| { id == working_id && n.label() == Some("Ready") })
    );
    ui.dismiss_notification(2);
    ui.dismiss_notification(2);
    assert!(ui.text.entry("@notification:2").is_none());
    assert_eq!(
        ui.next_notification_deadline(),
        Some(now + Duration::from_secs(4))
    );
    assert!(!ui.expire_notifications(now + Duration::from_secs(3)));
    assert!(ui.expire_notifications(now + Duration::from_secs(4)));
    ui.update_notification(
        1,
        "Must not return".into(),
        NotificationOptions::default(),
        now,
    );
    assert!(ui.draw(&mut assets, now).is_empty());
    assert!(ui.text.entry("@notification:1").is_none());

    ui.show_notification(
        3,
        "Expired".into(),
        NotificationOptions {
            duration: Some(0.0),
            ..NotificationOptions::default()
        },
        now,
    );
    ui.update_notification(
        3,
        "Must not return".into(),
        NotificationOptions::default(),
        now,
    );
    assert!(ui.draw(&mut assets, now).is_empty());
}

#[test]
fn visible_stack_and_long_messages_stay_inside_small_viewports() {
    let (mut ui, mut assets) = ui();
    let now = Instant::now();
    ui.set_surface_size(320.0, 180.0);
    for id in 1..=5 {
        ui.show_notification(
            id,
            if id == 5 {
                "Preferences updated".into()
            } else {
                format!("Notice {id}")
            },
            NotificationOptions::default(),
            now,
        );
    }
    ui.draw(&mut assets, now);
    assert_eq!(
        ui.notifications
            .iter()
            .filter(|n| n.bounds.is_some())
            .count(),
        4
    );
    assert!(ui.notifications[0].bounds.is_none());
    assert!(ui.notifications[1].bounds.is_some());
    let scale = ui.scale.clamp(0.75, 2.0);
    let bounds: Vec<_> = ui
        .notifications
        .iter()
        .rev()
        .filter_map(|n| n.bounds)
        .collect();
    assert!(f32::mul_add(-16.0, scale, bounds[0].y).abs() < 0.001);
    for pair in bounds.windows(2) {
        assert_eq!(pair[0].x, pair[1].x);
        assert_eq!(pair[0].w, pair[1].w);
        assert!((pair[1].y - pair[0].y - pair[0].h).abs() < 0.001);
    }
    let panels: Vec<_> = ui
        .draw(&mut assets, now)
        .into_iter()
        .filter_map(|item| {
            if let DrawItem::Quad(panel) = item {
                Some(panel)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(
        panels[0].radii,
        [panels[0].rect.h / 2.0, panels[0].rect.h / 2.0, 0.0, 0.0]
    );
    assert_eq!(panels[1].radii, [0.0; 4]);
    assert_eq!(panels[2].radii, [0.0; 4]);
    assert_eq!(
        panels[3].radii,
        [0.0, 0.0, panels[3].rect.h / 2.0, panels[3].rect.h / 2.0]
    );
    ui.dismiss_notification(5);
    ui.draw(&mut assets, now);
    assert!(ui.notifications[1].bounds.is_some());
    assert!(f32::mul_add(-16.0, scale, ui.notifications[3].bounds.unwrap().y).abs() < 0.001);
    ui.show_notification(
        6,
        "Unbroken_message_".repeat(100),
        NotificationOptions::default(),
        now,
    );
    let items = ui.draw(&mut assets, now);
    let clip = Rect {
        x: 0.0,
        y: 0.0,
        w: 320.0,
        h: 180.0,
    };
    for notification in &ui.notifications {
        if let Some(bounds) = notification.bounds {
            assert!(bounds.x >= clip.x && bounds.y >= clip.y);
            assert!(bounds.x + bounds.w <= clip.x + clip.w);
            assert!(bounds.y + bounds.h <= clip.y + clip.h);
        }
    }
    for item in items {
        if let DrawItem::Text(text) = item {
            assert!(text.clip.x >= clip.x && text.clip.x + text.clip.w <= clip.x + clip.w);
        }
    }
}
