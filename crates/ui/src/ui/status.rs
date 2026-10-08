use std::f32::consts::TAU;
use std::time::Instant;

use num_traits::AsPrimitive;

use super::desc::{Color, SpanDesc};
use super::text::{TextOptions, TextStyle};
use super::{DrawItem, Quad, Rect, TextDraw, Ui};

pub(super) const SELF_VOICING_STATUS: &str = "Enabling self-voicing…";
const TEXT_ID: &str = "@self-voicing-status";

impl Ui {
    pub fn set_self_voicing_initializing(&mut self, initializing: bool) {
        if initializing && self.self_voicing_initializing.is_none() {
            self.self_voicing_initializing = Some(Instant::now());
        } else if !initializing {
            self.self_voicing_initializing = None;
        }
    }

    pub(super) fn draw_self_voicing_status(&mut self, now: Instant, items: &mut Vec<DrawItem>) {
        let Some(started) = self.self_voicing_initializing else {
            return;
        };
        let clip = self.viewport();
        // Keep the status readable even when the game is scaled down.
        let scale = self.scale.clamp(0.75, 2.0);
        let max_width = (clip.w / scale - 88.0).max(1.0);
        let (text_width, text_height) = self.text.prepare(
            TEXT_ID,
            &[SpanDesc {
                text: SELF_VOICING_STATUS.into(),
                ..Default::default()
            }],
            &TextStyle {
                font_size: 20.0,
                line_height: 26.0,
                family: self.default_font.clone(),
                weight: 500,
                italic: false,
                align: None,
            },
            TextOptions {
                scale,
                width: Some(max_width),
                ..Default::default()
            },
        );
        let width = f32::mul_add(64.0, scale, text_width);
        let height = f32::mul_add(24.0, scale, text_height.max(28.0 * scale));
        let x = clip.x + (clip.w - width) / 2.0;
        let y = f32::mul_add(16.0, scale, clip.y);
        let quad = |rect, color, radius| {
            DrawItem::Quad(Quad {
                rect,
                color,
                radius,
                rotation: 0.0,
                border_width: 0.0,
                border_color: Color::WHITE,
                image: None,
                clip,
                mask: None,
            })
        };
        items.push(quad(
            Rect {
                x,
                y,
                w: width,
                h: height,
            },
            Color([0.08, 0.07, 0.10, 0.96]),
            10.0 * scale,
        ));
        let angle = now.saturating_duration_since(started).as_secs_f32() * TAU;
        let spinner_x = f32::mul_add(26.0, scale, x);
        let spinner_y = y + height / 2.0;
        let orbit = 10.0 * scale;
        for dot in 0..8 {
            let dot: f32 = dot.as_();
            let phase = angle - dot * TAU / 8.0;
            let radius = 2.5 * scale;
            items.push(quad(
                Rect {
                    x: phase.cos().mul_add(orbit, spinner_x) - radius,
                    y: phase.sin().mul_add(orbit, spinner_y) - radius,
                    w: radius * 2.0,
                    h: radius * 2.0,
                },
                Color([0.91, 0.66, 0.78, dot.mul_add(-0.1, 1.0)]),
                radius,
            ));
        }
        items.push(DrawItem::Text(TextDraw {
            id: TEXT_ID.into(),
            x: f32::mul_add(48.0, scale, x),
            y: y + (height - text_height) / 2.0,
            scale,
            color: Color::WHITE,
            clip,
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::super::desc::NodeDesc;
    use super::*;
    use std::collections::HashMap;
    use std::time::Duration;

    #[test]
    fn self_voicing_status_animates_survives_commits_and_clears() {
        let files = crate::GameFiles::open(&crate::workspace_dir().join("game")).unwrap();
        let mut ui = Ui::new(super::super::text::TextSystem::new(&files));
        let mut assets = crate::assets::Assets::new(files);
        let now = Instant::now();
        ui.set_self_voicing_initializing(true);
        assert!(ui.is_animating(now));
        let first = ui.draw(&mut assets, now);
        assert!(matches!(first.last(), Some(DrawItem::Text(text)) if text.id == TEXT_ID));
        let DrawItem::Quad(dot) = &first[1] else {
            panic!("Missing spinner")
        };
        let original = dot.rect;

        ui.commit(NodeDesc::default(), true, &HashMap::new(), &assets, now);
        let next = ui.draw(&mut assets, now + Duration::from_millis(250));
        let DrawItem::Quad(dot) = &next[1] else {
            panic!("Missing spinner")
        };
        assert_ne!(original, dot.rect);
        let tree = ui.accessibility_update("Test");
        assert!(tree.nodes.iter().any(|(_, node)| {
            node.label() == Some(SELF_VOICING_STATUS)
                && node.live() == Some(accesskit::Live::Polite)
        }));

        ui.set_surface_size(320.0, 180.0);
        let small = ui.draw(&mut assets, now);
        let DrawItem::Quad(panel) = &small[0] else {
            panic!("Missing status panel")
        };
        assert!(panel.rect.x >= ui.viewport().x);
        assert!(panel.rect.x + panel.rect.w <= ui.viewport().x + ui.viewport().w);

        ui.set_self_voicing_initializing(false);
        assert!(!ui.is_animating(now));
        assert!(ui.draw(&mut assets, now).is_empty());
        assert!(
            !ui.accessibility_update("Test")
                .nodes
                .iter()
                .any(|(_, node)| { node.label() == Some(SELF_VOICING_STATUS) })
        );
    }
}
