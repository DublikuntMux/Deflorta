use std::f32::consts::TAU;
use std::time::Instant;

use deflorta_common::notification::{NotificationId, NotificationOptions, NotificationState};
use glyphon::cosmic_text::{Ellipsize, EllipsizeHeightLimit, Wrap};
use num_traits::AsPrimitive;

use super::desc::{Color, SpanDesc};
use super::text::{TextOptions, TextStyle};
use super::{DrawItem, Quad, Rect, TextDraw, Ui};

#[cfg(test)]
mod tests;

pub(super) struct Notification {
    pub id: NotificationId,
    pub message: String,
    pub state: NotificationState,
    pub text_id: String,
    pub bounds: Option<Rect>,
    started: Instant,
    expires: Option<Instant>,
}

struct NotificationRow {
    index: usize,
    width: f32,
    height: f32,
    text_height: f32,
    icon_space: f32,
}

impl Ui {
    pub fn show_notification(
        &mut self,
        id: NotificationId,
        message: String,
        options: NotificationOptions,
        now: Instant,
    ) {
        self.dismiss_notification(id);
        self.notifications.push(Notification {
            id,
            message,
            state: options.state,
            text_id: format!("@notification:{id}"),
            bounds: None,
            started: now,
            expires: options.deadline(now).ok().flatten(),
        });
    }

    pub fn update_notification(
        &mut self,
        id: NotificationId,
        message: String,
        options: NotificationOptions,
        now: Instant,
    ) {
        let Some(notification) = self.notifications.iter_mut().find(|n| n.id == id) else {
            return;
        };
        // Expired handles cannot bring a notification back, even before the next poll.
        if notification.expires.is_some_and(|due| now >= due) {
            self.dismiss_notification(id);
            return;
        }
        if notification.state != options.state {
            notification.started = now;
        }
        notification.message = message;
        notification.state = options.state;
        notification.expires = options.deadline(now).ok().flatten();
    }

    pub fn dismiss_notification(&mut self, id: NotificationId) {
        self.notifications.retain(|notification| {
            if notification.id == id {
                self.text.remove(&notification.text_id);
                false
            } else {
                true
            }
        });
    }

    pub fn expire_notifications(&mut self, now: Instant) -> bool {
        let count = self.notifications.len();
        self.notifications.retain(|notification| {
            if notification.expires.is_some_and(|due| now >= due) {
                self.text.remove(&notification.text_id);
                false
            } else {
                true
            }
        });
        count != self.notifications.len()
    }

    #[must_use]
    pub fn next_notification_deadline(&self) -> Option<Instant> {
        self.notifications.iter().filter_map(|n| n.expires).min()
    }

    pub(super) fn draw_notifications(&mut self, now: Instant, items: &mut Vec<DrawItem>) {
        self.expire_notifications(now);
        for notification in &mut self.notifications {
            notification.bounds = None;
        }
        let clip = self.surface_bounds;
        // Native overlays stay readable when a large game canvas is scaled down.
        let scale = self.scale.clamp(0.75, 2.0);
        let rows = self.layout_notifications(scale);
        let width = rows.iter().map(|row| row.width).fold(0.0, f32::max);
        let count = rows.len();
        let x = clip.x + (clip.w - width) / 2.0;
        let mut y = f32::mul_add(16.0, scale, clip.y);
        for (position, row) in rows.into_iter().enumerate() {
            let notification = &mut self.notifications[row.index];
            let rect = Rect {
                x,
                y,
                w: width,
                h: row.height,
            };
            notification.bounds = Some(rect);
            let top = if position == 0 { row.height / 2.0 } else { 0.0 };
            let bottom = if position + 1 == count {
                row.height / 2.0
            } else {
                0.0
            };
            let mut panel = quad(rect, Color([0.11, 0.10, 0.14, 0.98]), 0.0, clip);
            panel.radii = [top, top, bottom, bottom];
            panel.border_width = scale;
            panel.border_color = Color([0.95, 0.85, 0.92, 0.15]);
            items.push(DrawItem::Quad(panel));
            draw_icon(
                notification,
                f32::mul_add(30.0, scale, x),
                y + row.height / 2.0,
                scale,
                now,
                clip,
                items,
            );
            let text_x = f32::mul_add(20.0, scale, x) + row.icon_space;
            items.push(DrawItem::Text(TextDraw {
                id: notification.text_id.clone(),
                x: text_x,
                y: y + (row.height - row.text_height) / 2.0,
                // The text buffer is already shaped at the physical display scale.
                scale: 1.0,
                color: Color::WHITE,
                clip: Rect {
                    x: text_x,
                    y: f32::mul_add(12.0, scale, y),
                    w: f32::mul_add(-40.0, scale, width - row.icon_space).max(0.0),
                    h: f32::mul_add(-24.0, scale, row.height).max(0.0),
                }
                .intersect(&clip),
            }));
            y += row.height;
        }
    }

    fn layout_notifications(&mut self, scale: f32) -> Vec<NotificationRow> {
        let clip = self.surface_bounds;
        let padding = 20.0 * scale;
        let horizontal_padding = padding * 2.0;
        let max_width = f32::mul_add(-32.0, scale, clip.w)
            .max(1.0)
            .min(560.0 * scale);
        let available_height = f32::mul_add(-32.0, scale, clip.h).max(1.0);
        let style = TextStyle {
            font_size: 18.0,
            line_height: 1.4,
            family: self.default_font.clone(),
            weight: 500,
            italic: false,
            align: None,
        };
        let mut rows = Vec::new();
        let mut total_height = 0.0;
        let text_limit = f32::mul_add(-24.0, scale, available_height)
            .min(3.0 * style.font_size * style.line_height * scale)
            .max(1.0);
        // Show the newest notifications that fit; hidden handles keep their state and lifetime.
        for (index, notification) in self.notifications.iter().enumerate().rev() {
            let icon_space = if notification.state == NotificationState::Info {
                0.0
            } else {
                32.0 * scale
            };
            let text_width = (max_width - horizontal_padding - icon_space).max(1.0);
            let (width, height) = self.text.prepare(
                &notification.text_id,
                &[SpanDesc {
                    text: notification.message.clone(),
                    ..SpanDesc::default()
                }],
                &style,
                TextOptions {
                    scale,
                    width: Some(text_width / scale),
                    wrap: Wrap::WordOrGlyph,
                    ellipsize: Ellipsize::End(EllipsizeHeightLimit::Height(text_limit)),
                    ..TextOptions::default()
                },
            );
            let text_height = height;
            let height =
                f32::mul_add(24.0, scale, text_height.max(28.0 * scale)).min(available_height);
            let width = (width.ceil() + horizontal_padding + icon_space).min(max_width);
            let next_height = total_height + height;
            if next_height > available_height {
                break;
            }
            rows.push(NotificationRow {
                index,
                width,
                height,
                text_height,
                icon_space,
            });
            total_height = next_height;
        }
        rows
    }
}

const fn quad(rect: Rect, color: Color, radius: f32, clip: Rect) -> Quad {
    Quad {
        rect,
        color,
        radii: [radius; 4],
        rotation: 0.0,
        border_width: 0.0,
        border_color: Color::TRANSPARENT,
        image: None,
        clip,
        mask: None,
    }
}

fn draw_icon(
    notification: &Notification,
    x: f32,
    y: f32,
    scale: f32,
    now: Instant,
    clip: Rect,
    items: &mut Vec<DrawItem>,
) {
    let color = match notification.state {
        NotificationState::Info => return,
        NotificationState::Loading => Color([0.91, 0.66, 0.78, 1.0]),
        NotificationState::Success => Color([0.48, 0.83, 0.67, 1.0]),
        NotificationState::Error => Color([0.97, 0.48, 0.49, 1.0]),
    };
    let line = |a: (f32, f32), b: (f32, f32)| {
        let dx = (b.0 - a.0) * scale;
        let dy = (b.1 - a.1) * scale;
        let width = dx.hypot(dy);
        let height = 2.5 * scale;
        let mut mark = quad(
            Rect {
                x: f32::mul_add(a.0.midpoint(b.0), scale, x) - width / 2.0,
                y: f32::mul_add(a.1.midpoint(b.1), scale, y) - height / 2.0,
                w: width,
                h: height,
            },
            color,
            height / 2.0,
            clip,
        );
        mark.rotation = dy.atan2(dx);
        DrawItem::Quad(mark)
    };
    match notification.state {
        NotificationState::Loading => {
            let angle = now
                .saturating_duration_since(notification.started)
                .as_secs_f32()
                * TAU;
            for dot in 0..8 {
                let dot: f32 = dot.as_();
                let phase = angle - dot * TAU / 8.0;
                let radius = 2.0 * scale;
                items.push(DrawItem::Quad(quad(
                    Rect {
                        x: phase.cos().mul_add(8.0 * scale, x) - radius,
                        y: phase.sin().mul_add(8.0 * scale, y) - radius,
                        w: radius * 2.0,
                        h: radius * 2.0,
                    },
                    color.with_alpha_mul(dot.mul_add(-0.1, 1.0)),
                    radius,
                    clip,
                )));
            }
        }
        NotificationState::Success => {
            items.push(line((-7.0, 0.0), (-2.0, 5.0)));
            items.push(line((-2.0, 5.0), (8.0, -6.0)));
        }
        NotificationState::Error => {
            items.push(line((0.0, -7.0), (0.0, 2.0)));
            let radius = 1.5 * scale;
            items.push(DrawItem::Quad(quad(
                Rect {
                    x: x - radius,
                    y: f32::mul_add(6.0, scale, y) - radius,
                    w: radius * 2.0,
                    h: radius * 2.0,
                },
                color,
                radius,
                clip,
            )));
        }
        NotificationState::Info => {}
    }
}
