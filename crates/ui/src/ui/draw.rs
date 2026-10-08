use std::sync::Arc;
use std::time::Instant;

use num_traits::AsPrimitive;

use super::desc::{Color, Ease, Fit, MaskDesc, NodeKind, Overflow, SpanDesc, Style, WipeDir};
use super::layout::input_display;
use super::text::TextOptions;
use super::transform::{self, Similarity, Values};
use super::{DrawItem, ImageRef, MaskDraw, Quad, Rect, TextDraw, Timed, Ui};
use crate::assets::Assets;
use crate::ui::desc::{Display, Edges};
use crate::util::math::lerp;
use crate::util::time::elapsed_secs;

const DEFAULT_TRACK: Color = Color([1.0, 1.0, 1.0, 0.18]);
const DEFAULT_FILL: Color = Color([0.91, 0.66, 0.78, 1.0]);
const DEFAULT_SCROLLBAR: Color = Color([1.0, 1.0, 1.0, 0.3]);

#[derive(Clone)]
struct Ctx {
    xf: Similarity,
    opacity: f32,
    color: Color,
    clip: Rect,
    mask: Option<MaskDraw>,
}

fn mask_draw(spec: Option<&MaskDesc>, progress: f32, invert: bool) -> Option<MaskDraw> {
    let (kind, param, src) = match spec? {
        MaskDesc::Image { src, ramp } => (1, *ramp, Some(src.clone())),
        MaskDesc::Wipe { dir, ramp } => {
            let kind = match dir {
                WipeDir::Left => 2,
                WipeDir::Right => 3,
                WipeDir::Up => 4,
                WipeDir::Down => 5,
            };
            (kind, *ramp, None)
        }
        MaskDesc::Pixellate { size } => (6, *size, None),
    };
    Some(MaskDraw {
        kind,
        progress,
        param,
        invert,
        src,
    })
}

impl Ui {
    pub fn draw(&mut self, assets: &mut Assets, now: Instant) -> Vec<DrawItem> {
        self.layout_if_needed(assets, now);
        self.hit_order.clear();
        let mut items = Vec::new();
        if self.nodes.is_empty() {
            return items;
        }
        let ctx = Ctx {
            xf: Similarity::new(self.scale, self.offset.0, self.offset.1),
            opacity: 1.0,
            color: Color::WHITE,
            clip: self.viewport(),
            mask: None,
        };
        self.draw_node(0, &ctx, assets, now, &mut items);
        if self
            .focused
            .as_ref()
            .and_then(|key| self.index.get(key))
            .is_some_and(|&i| {
                !self.nodes[i].desc.is_focusable()
                    || self
                        .ancestors(i)
                        .any(|a| self.nodes[a].desc.style.display == Some(Display::None))
                    || self.hit_order.iter().any(|(n, _)| *n == i) && !self.accessibility_exposed(i)
            })
        {
            self.focused = None;
        }
        items
    }

    fn is_highlighted(&self, i: usize) -> bool {
        self.hovered.contains(&i) || self.focused.as_deref() == Some(self.nodes[i].id.as_str())
    }

    fn draw_node(
        &mut self,
        i: usize,
        parent: &Ctx,
        assets: &mut Assets,
        now: Instant,
        items: &mut Vec<DrawItem>,
    ) {
        let node = &self.nodes[i];
        if node.tooltip_hidden || node.desc.style.display == Some(Display::None) {
            return;
        }
        let highlighted = self.is_highlighted(i);
        let hover = if highlighted {
            node.desc.hover.as_ref()
        } else {
            None
        };
        let pick = |f: fn(&Style) -> Option<f32>| hover.and_then(f).or_else(|| f(&node.desc.style));
        let pick_color =
            |f: fn(&Style) -> Option<Color>| hover.and_then(f).or_else(|| f(&node.desc.style));

        let Some((anim, mask, program)) = self.draw_animation(i, parent.mask.as_ref(), now) else {
            return;
        };

        let opacity =
            parent.opacity * anim.opacity * program.opacity * pick(|s| s.opacity).unwrap_or(1.0);
        if opacity <= 0.001 {
            return;
        }
        let rect = node.rect;
        let k = anim.scale * program.scale * pick(|s| s.scale).unwrap_or(1.0);
        let degrees = anim.rotate + program.rotate + node.desc.style.rotate.unwrap_or(0.0);
        let [ax, ay] = node.desc.anchor.unwrap_or([0.5, 0.5]);
        let local = Similarity::local(
            k,
            degrees.to_radians(),
            f32::mul_add(ax, rect.w, rect.x),
            f32::mul_add(ay, rect.h, rect.y),
            anim.x + program.x,
            anim.y + program.y,
        );
        let xf = parent.xf.then(&local);
        let bounds = xf.bounds(rect);
        let visible = bounds.intersect(&parent.clip);
        if !node.in_ghost {
            self.hit_order.push((i, visible));
        }

        let color = pick_color(|s| s.color).unwrap_or(parent.color);
        let ctx = Ctx {
            xf,
            opacity,
            color,
            clip: parent.clip,
            mask,
        };
        self.draw_content(i, &ctx, program.crop, assets, now, items);
        self.draw_children(i, &ctx, bounds, assets, now, items);
    }

    fn draw_animation(
        &self,
        i: usize,
        inherited_mask: Option<&MaskDraw>,
        now: Instant,
    ) -> Option<(Values, Option<MaskDraw>, Values)> {
        let node = &self.nodes[i];
        let mut anim = Values::default();
        let mut mask = inherited_mask.cloned();
        let mut apply_timed = |timed: &Timed, exiting: bool| {
            let e = timed.progress(now, Ease::EaseOut);
            let s = &timed.spec;
            let k = if exiting { e } else { 1.0 - e };
            anim.opacity = lerp(1.0, s.opacity.unwrap_or(1.0), k);
            anim.x = lerp(0.0, s.x.unwrap_or(0.0), k);
            anim.y = lerp(0.0, s.y.unwrap_or(0.0), k);
            anim.scale = lerp(1.0, s.scale.unwrap_or(1.0), k);
            anim.rotate = lerp(0.0, s.rotate.unwrap_or(0.0), k);
            if let Some(m) = mask_draw(s.mask.as_ref(), e, exiting) {
                mask = Some(m);
            }
        };
        if let Some(ghost) = &node.ghost {
            if ghost.finished(now) {
                return None;
            }
            apply_timed(ghost, true);
        } else if let Some(enter) = self.enters.get(&node.id) {
            apply_timed(enter, false);
        }
        if let Some(m) = self.moves.get(&node.id) {
            let e = Timed {
                start: m.start,
                spec: m.spec.clone(),
            }
            .progress(now, Ease::EaseInOut);
            anim.x = f32::mul_add(m.dx, 1.0 - e, anim.x);
            anim.y = f32::mul_add(m.dy, 1.0 - e, anim.y);
        }
        let mut program = Values::default();
        if let Some((start, desc)) = self.transforms.get(&node.id) {
            transform::evaluate(&desc.steps, elapsed_secs(*start, now), &mut program);
        }
        Some((anim, mask, program))
    }

    fn draw_content(
        &mut self,
        i: usize,
        ctx: &Ctx,
        crop_rect: [f32; 4],
        assets: &mut Assets,
        now: Instant,
        items: &mut Vec<DrawItem>,
    ) {
        let node = &self.nodes[i];
        let highlighted = self.is_highlighted(i);
        let hover = highlighted.then_some(node.desc.hover.as_ref()).flatten();
        let pick_color =
            |f: fn(&Style) -> Option<Color>| hover.and_then(f).or_else(|| f(&node.desc.style));
        let rect = node.rect;
        let scale = ctx.xf.scale();
        let radius = node.desc.style.radius.unwrap_or(0.0) * scale;
        let border_width = node.desc.style.border_width.unwrap_or(0.0) * scale;
        let border_color = pick_color(|s| s.border_color).unwrap_or(Color::TRANSPARENT);
        let quad = |r: Rect, color: Color, radius: f32| {
            let (rect, rotation) = ctx.xf.rect(r);
            Quad {
                rect,
                rotation,
                color: color.with_alpha_mul(ctx.opacity),
                radius: radius * scale,
                border_width: 0.0,
                border_color: Color::TRANSPARENT,
                image: None,
                clip: ctx.clip,
                mask: ctx.mask.clone(),
            }
        };
        let background = pick_color(|s| s.background);
        if background.is_some() || border_width > 0.0 {
            let mut q = quad(rect, background.unwrap_or(Color::TRANSPARENT), 0.0);
            q.radius = radius;
            q.border_width = border_width;
            q.border_color = border_color.with_alpha_mul(ctx.opacity);
            items.push(DrawItem::Quad(q));
        }

        let kind = node.desc.kind();
        match kind {
            NodeKind::Image | NodeKind::Video => {
                let source = match kind {
                    NodeKind::Image => {
                        let src = if highlighted {
                            node.desc.hover_src.as_ref().or(node.desc.src.as_ref())
                        } else {
                            node.desc.src.as_ref()
                        };
                        src.and_then(|src| {
                            assets.image_size(src).map(|size| (src.clone(), size, None))
                        })
                    }
                    _ => self.videos.get(&node.id).and_then(|player| {
                        let (serial, frame) = player.frame(now)?;
                        Some((
                            format!("video:{}", node.id),
                            frame.dimensions(),
                            Some((serial, frame)),
                        ))
                    }),
                };
                if let Some((src, (iw, ih), frame)) = source {
                    let (dest, uv) = fit_image(rect, iw.as_(), ih.as_(), node.desc.fit);
                    let (dest, uv) = crop(dest, uv, crop_rect);
                    let mut q = quad(dest, Color::WHITE, 0.0);
                    q.radius = radius;
                    q.image = Some(ImageRef { src, uv, frame });
                    items.push(DrawItem::Quad(q));
                }
            }
            NodeKind::Slider => {
                self.draw_slider(i, hover, &quad, items);
            }
            NodeKind::Text | NodeKind::Input => {
                self.draw_text(i, ctx, now, items);
            }
            NodeKind::Box => {}
        }
    }

    fn draw_slider(
        &self,
        i: usize,
        hover: Option<&Style>,
        quad: &impl Fn(Rect, Color, f32) -> Quad,
        items: &mut Vec<DrawItem>,
    ) {
        let node = &self.nodes[i];
        let rect = node.rect;
        let pick_color =
            |f: fn(&Style) -> Option<Color>| hover.and_then(f).or_else(|| f(&node.desc.style));
        let min = node.desc.min.unwrap_or(0.0);
        let max = node.desc.max.unwrap_or(1.0);
        let frac = if max > min {
            ((node.desc.number_value() - min) / (max - min)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let thumb = node.desc.style.thumb_size.unwrap_or(rect.h);
        let track_h = (rect.h * 0.3).max(4.0);
        let track = Rect {
            x: rect.x,
            y: rect.y + (rect.h - track_h) / 2.0,
            w: rect.w,
            h: track_h,
        };
        if pick_color(|s| s.background).is_none() {
            items.push(DrawItem::Quad(quad(track, DEFAULT_TRACK, track_h / 2.0)));
        }
        let fill = pick_color(|s| s.fill_color).unwrap_or(DEFAULT_FILL);
        items.push(DrawItem::Quad(quad(
            Rect {
                w: rect.w * frac,
                ..track
            },
            fill,
            track_h / 2.0,
        )));
        let thumb_color = pick_color(|s| s.thumb_color).unwrap_or(Color::WHITE);
        let thumb_rect = Rect {
            x: rect.w.mul_add(frac, rect.x) - thumb / 2.0,
            y: rect.y + (rect.h - thumb) / 2.0,
            w: thumb,
            h: thumb,
        };
        items.push(DrawItem::Quad(quad(thumb_rect, thumb_color, thumb / 2.0)));
    }

    fn draw_children(
        &mut self,
        i: usize,
        parent: &Ctx,
        bounds: Rect,
        assets: &mut Assets,
        now: Instant,
        items: &mut Vec<DrawItem>,
    ) {
        let node = &self.nodes[i];
        let rect = node.rect;
        let xf = parent.xf;
        let overflow = node.desc.style.overflow;
        let clips = matches!(overflow, Some(Overflow::Hidden | Overflow::Scroll));
        let child_clip = if clips {
            parent.clip.intersect(&bounds)
        } else {
            parent.clip
        };
        if child_clip.w <= 0.0 || child_clip.h <= 0.0 {
            return;
        }
        let (sx, sy) = self.scroll.get(&node.id).copied().unwrap_or((0.0, 0.0));
        let child_xf = if sx != 0.0 || sy != 0.0 {
            xf.then(&Similarity::new(1.0, -sx, -sy))
        } else {
            xf
        };
        let ctx = Ctx {
            xf: child_xf,
            opacity: parent.opacity,
            color: parent.color,
            clip: child_clip,
            mask: parent.mask.clone(),
        };
        let children = node.children.clone();
        let content = node.content;
        for c in children {
            self.draw_node(c, &ctx, assets, now, items);
        }

        if overflow == Some(Overflow::Scroll) && content.1 > rect.h + 0.5 {
            let bar_w = 6.0;
            let track_h = rect.h;
            let bar_h = (track_h * rect.h / content.1).max(24.0);
            let y = (track_h - bar_h).mul_add(sy / (content.1 - rect.h), rect.y);
            let bar = Rect {
                x: rect.x + rect.w - bar_w - 2.0,
                y,
                w: bar_w,
                h: bar_h,
            };
            let color = self.nodes[i]
                .desc
                .style
                .scrollbar_color
                .unwrap_or(DEFAULT_SCROLLBAR);
            let (r, rotation) = xf.rect(bar);
            items.push(DrawItem::Quad(Quad {
                rect: r,
                rotation,
                color: color.with_alpha_mul(parent.opacity),
                radius: bar_w / 2.0 * xf.scale(),
                border_width: 0.0,
                border_color: Color::TRANSPARENT,
                image: None,
                clip: parent.clip,
                mask: None,
            }));
        }
    }

    fn draw_text(&mut self, i: usize, ctx: &Ctx, now: Instant, items: &mut Vec<DrawItem>) {
        let (xf, opacity, color, clip) = (&ctx.xf, ctx.opacity, ctx.color, ctx.clip);
        let node = &self.nodes[i];
        let id = node.id.clone();
        let style = &node.text_style;
        let shadow = node.desc.style.text_shadow;
        let [pt, pr, _, pl] = node.desc.style.padding.map_or([0.0; 4], Edges::trbl);
        let border = node.desc.style.border_width.unwrap_or(0.0);
        let rect = Rect {
            x: node.rect.x + pl + border,
            y: node.rect.y + pt + border,
            w: 2.0f32.mul_add(-border, node.rect.w - pl - pr).max(0.0),
            h: node.rect.h,
        };
        let is_input = node.desc.kind() == NodeKind::Input;
        let (spans, color, width) = node.spans.as_ref().map_or_else(
            || {
                let (content, placeholder) = input_display(node);
                let color = if placeholder {
                    color.with_alpha_mul(0.45)
                } else {
                    color
                };
                (
                    Arc::new(vec![SpanDesc {
                        text: content,
                        ..Default::default()
                    }]),
                    color,
                    None,
                )
            },
            |spans| (Arc::clone(spans), color, Some(rect.w)),
        );
        let revealed = self.reveals.get(&id).map_or(usize::MAX, |r| r.shown(now));
        let options = TextOptions {
            scale: self.scale,
            width,
            revealed,
            alpha: opacity,
            ..TextOptions::default()
        };
        let (text_w, _) = self.text.prepare(&id, &spans, style, options);
        let text_scale = xf.scale() / self.scale;
        let (x, y) = xf.apply(rect.x, rect.y);

        if let Some(shadow) = shadow {
            let shadow_id = format!("{id}#shadow");
            self.text.prepare(
                &shadow_id,
                &spans,
                style,
                TextOptions {
                    color_override: Some(shadow.color),
                    ..options
                },
            );
            items.push(DrawItem::Text(TextDraw {
                id: shadow_id,
                x: f32::mul_add(shadow.x, xf.scale(), x),
                y: f32::mul_add(shadow.y, xf.scale(), y),
                scale: text_scale,
                color: shadow.color.with_alpha_mul(opacity),
                clip,
            }));
        }
        let draw = TextDraw {
            id: id.clone(),
            x,
            y,
            scale: text_scale,
            color: color.with_alpha_mul(opacity),
            clip,
        };
        self.draw_text_items(draw, opacity, items);

        if is_input && self.focused.as_deref() == Some(id.as_str()) {
            let value_empty = self.nodes[i].desc.string_value().is_empty();
            let caret_x = if value_empty {
                0.0
            } else {
                text_w * text_scale
            };
            let line = style.font_size * style.line_height * xf.scale();
            items.push(DrawItem::Quad(Quad {
                rect: Rect {
                    x: x + caret_x + 1.0,
                    y: line.mul_add(0.1, y),
                    w: (2.0 * xf.scale()).max(1.0),
                    h: line * 0.8,
                },
                rotation: 0.0,
                color: color.with_alpha_mul(opacity),
                radius: 0.0,
                border_width: 0.0,
                border_color: Color::TRANSPARENT,
                image: None,
                clip,
                mask: None,
            }));
        }
    }

    fn draw_text_items(&self, draw: TextDraw, opacity: f32, items: &mut Vec<DrawItem>) {
        let entry = self.text.entry(&draw.id);
        let TextDraw {
            x,
            y,
            scale,
            color,
            clip,
            ..
        } = draw;
        items.push(DrawItem::Text(draw));
        let Some(entry) = entry else {
            return;
        };
        for d in &entry.decorations {
            items.push(DrawItem::Quad(Quad {
                rect: Rect {
                    x: f32::mul_add(d.x, scale, x),
                    y: f32::mul_add(d.y, scale, y),
                    w: d.w * scale,
                    h: d.h * scale,
                },
                rotation: 0.0,
                color: d.color.map_or(color, |color| color.with_alpha_mul(opacity)),
                radius: 0.0,
                border_width: 0.0,
                border_color: Color::TRANSPARENT,
                image: None,
                clip,
                mask: None,
            }));
        }
        for ruby in &entry.rubies {
            items.push(DrawItem::Text(TextDraw {
                id: ruby.id.clone(),
                x: f32::mul_add(ruby.x, scale, x),
                y: f32::mul_add(ruby.y, scale, y),
                scale,
                color,
                clip,
            }));
        }
    }
}

fn fit_image(rect: Rect, iw: f32, ih: f32, fit: Fit) -> (Rect, [f32; 4]) {
    if rect.w <= 0.0 || rect.h <= 0.0 || iw <= 0.0 || ih <= 0.0 {
        return (rect, [0.0, 0.0, 1.0, 1.0]);
    }
    let box_aspect = rect.w / rect.h;
    let img_aspect = iw / ih;
    match fit {
        Fit::Fill => (rect, [0.0, 0.0, 1.0, 1.0]),
        Fit::Cover => {
            if img_aspect > box_aspect {
                let u = box_aspect / img_aspect;
                (rect, [(1.0 - u) / 2.0, 0.0, f32::midpoint(1.0, u), 1.0])
            } else {
                let v = img_aspect / box_aspect;
                (rect, [0.0, (1.0 - v) / 2.0, 1.0, f32::midpoint(1.0, v)])
            }
        }
        Fit::Contain => {
            let (w, h) = if img_aspect > box_aspect {
                (rect.w, rect.w / img_aspect)
            } else {
                (rect.h * img_aspect, rect.h)
            };
            (
                Rect {
                    x: rect.x + (rect.w - w) / 2.0,
                    y: rect.y + (rect.h - h) / 2.0,
                    w,
                    h,
                },
                [0.0, 0.0, 1.0, 1.0],
            )
        }
    }
}

// The exact identity crop bypasses arithmetic to preserve the original bounds.
fn crop(dest: Rect, uv: [f32; 4], c: [f32; 4]) -> (Rect, [f32; 4]) {
    if c.iter()
        .zip([0.0, 0.0, 1.0, 1.0])
        .all(|(actual, expected)| actual.partial_cmp(&expected) == Some(std::cmp::Ordering::Equal))
    {
        return (dest, uv);
    }
    let [cx, cy, cw, ch] = c.map(|v| v.clamp(0.0, 1.0));
    let (uw, vh) = (uv[2] - uv[0], uv[3] - uv[1]);
    let uv = [
        f32::mul_add(cx, uw, uv[0]),
        f32::mul_add(cy, vh, uv[1]),
        f32::mul_add(cx + cw, uw, uv[0]),
        f32::mul_add(cy + ch, vh, uv[1]),
    ];
    let dest = Rect {
        x: f32::mul_add(cx, dest.w, dest.x),
        y: f32::mul_add(cy, dest.h, dest.y),
        w: dest.w * cw,
        h: dest.h * ch,
    };
    (dest, uv)
}
