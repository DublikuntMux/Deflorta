use std::collections::HashMap;
use std::collections::HashSet;
use std::time::Instant;

use log::error;
use num_traits::AsPrimitive;
use taffy as tf;

use super::desc::{
    Align, Dim, Display, Edges, FlexDirection, FlexWrap, NodeKind, Overflow, Position, SpanDesc,
    Style,
};
use super::reveal::Reveal;
use super::text::TextSystem;
use super::{Move, Node, Rect, Ui};
use crate::assets::Assets;
use crate::video::VideoPlayer;

const SLIDER_HEIGHT: f32 = 24.0;

impl Ui {
    pub(super) fn sync_tooltip_text(&mut self) {
        let content = self.tooltip.as_deref().unwrap_or_default();
        for (i, node) in self.nodes.iter_mut().enumerate() {
            if node.in_ghost || node.desc.kind() != NodeKind::Text || !node.desc.tooltip_text {
                continue;
            }
            node.spans = Some(std::sync::Arc::new(vec![SpanDesc {
                text: content.to_owned(),
                ..Default::default()
            }]));
            node.tooltip_hidden = content.is_empty();
            if let Some(id) = node.layout_id {
                let style = layout_style(node, (i == 0).then_some(self.virtual_size));
                if self.layout_tree.style(id).expect("live tooltip node") != &style {
                    self.layout_tree
                        .set_style(id, style)
                        .expect("update tooltip visibility");
                }
                self.layout_tree
                    .mark_dirty(id)
                    .expect("invalidate tooltip measurement");
            }
            self.layout_dirty = true;
        }
    }

    pub(super) fn reconcile_layout(&mut self, old: &[Node], old_index: &HashMap<String, usize>) {
        let tree = &mut self.layout_tree;
        let mut retained = HashSet::new();
        for (i, node) in self
            .nodes
            .iter_mut()
            .enumerate()
            .filter(|(_, n)| !n.in_ghost)
        {
            let previous = old_index
                .get(&node.id)
                .map(|&oi| &old[oi])
                .filter(|old| old.desc.kind() == node.desc.kind());
            let style = layout_style(node, (i == 0).then_some(self.virtual_size));
            let id = if let Some(old) = previous.filter(|old| old.layout_id.is_some()) {
                let id = old.layout_id.unwrap();
                retained.insert(id);
                // Context indices change on insertion/reordering; updating them
                // through this accessor preserves Taffy's measurement cache.
                *tree.get_node_context_mut(id).expect("live layout context") = i;
                if tree.style(id).expect("live layout node") != &style {
                    tree.set_style(id, style).expect("update layout style");
                }
                if measurement_changed(old, node) {
                    tree.mark_dirty(id).expect("invalidate measurement");
                }
                node.rect = old.rect;
                node.content = old.content;
                self.layout_dirty |= old.desc.anchor != node.desc.anchor
                    || old.desc.start_at_end != node.desc.start_at_end;
                id
            } else {
                self.layout_dirty = true;
                tree.new_leaf_with_context(style, i)
                    .expect("create layout node")
            };
            node.layout_id = Some(id);
            if tree.dirty(id).expect("live layout node") {
                self.layout_dirty = true;
            }
        }
        for node in self.nodes.iter().filter(|n| !n.in_ghost) {
            let id = node.layout_id.expect("live layout node");
            let children: Vec<_> = node
                .children
                .iter()
                .filter_map(|&c| self.nodes[c].layout_id)
                .collect();
            if tree.children(id).expect("live layout node") != children {
                tree.set_children(id, &children)
                    .expect("update layout children");
                self.layout_dirty = true;
            }
        }
        for id in old
            .iter()
            .filter_map(|n| n.layout_id)
            .filter(|id| !retained.contains(id))
        {
            // Taffy's remove() does not release the user context itself.
            tree.set_node_context(id, None)
                .expect("release layout context");
            tree.remove(id).expect("remove layout node");
            self.layout_dirty = true;
        }
    }

    pub(super) fn invalidate_measurements(&mut self) {
        for id in self.nodes.iter().filter_map(|n| n.layout_id) {
            self.layout_tree
                .mark_dirty(id)
                .expect("invalidate measurement");
        }
    }

    pub(super) fn layout_if_needed(&mut self, assets: &mut Assets, now: Instant) {
        if !self.layout_dirty || self.nodes.is_empty() {
            return;
        }
        let (vw, vh) = self.virtual_size;
        let root = self.nodes[0].layout_id.expect("live layout root");
        let style = layout_style(&self.nodes[0], Some(self.virtual_size));
        if self.layout_tree.style(root).expect("live layout root") != &style {
            self.layout_tree
                .set_style(root, style)
                .expect("update root size");
        }

        let scale = self.scale;
        let nodes = &self.nodes;
        let reveals = &self.reveals;
        let videos = &self.videos;
        let text = &mut self.text;
        let result = self.layout_tree.compute_layout_with_measure(
            root,
            tf::Size {
                width: tf::AvailableSpace::Definite(vw),
                height: tf::AvailableSpace::Definite(vh),
            },
            |inputs, _id, ctx, style| {
                tf::compute_leaf_layout(
                    inputs,
                    style,
                    |_, _| 0.0,
                    |known, available| {
                        let Some(&mut i) = ctx else {
                            return tf::Size::ZERO;
                        };
                        measure(
                            &nodes[i], known, available, scale, text, assets, reveals, videos, now,
                        )
                    },
                )
            },
        );
        if let Err(err) = result {
            error!("Layout failed: {err}");
            return;
        }
        self.layout_dirty = false;
        Self::assign_rects(&self.layout_tree, &mut self.nodes, root, 0, 0.0, 0.0);

        for (id, (old, spec)) in std::mem::take(&mut self.pending_moves) {
            let Some(&i) = self.index.get(&id) else {
                continue;
            };
            let new = self.nodes[i].rect;
            let (dx, dy) = (old.x - new.x, old.y - new.y);
            if dx.abs() > 0.5 || dy.abs() > 0.5 {
                self.moves.insert(
                    id,
                    Move {
                        start: now,
                        spec,
                        dx,
                        dy,
                    },
                );
            }
        }

        for node in &self.nodes {
            if node.desc.start_at_end && !node.in_ghost && !self.scroll.contains_key(&node.id) {
                let max = (node.content.1 - node.rect.h).max(0.0);
                self.scroll.insert(node.id.clone(), (0.0, max));
            }
        }

        self.scroll.iter_mut().for_each(|(id, (sx, sy))| {
            if let Some(&i) = self.index.get(id) {
                let node = &self.nodes[i];
                *sx = sx.clamp(0.0, (node.content.0 - node.rect.w).max(0.0));
                *sy = sy.clamp(0.0, (node.content.1 - node.rect.h).max(0.0));
            }
        });
    }

    fn assign_rects(
        tree: &tf::TaffyTree<usize>,
        nodes: &mut [Node],
        tid: tf::NodeId,
        i: usize,
        px: f32,
        py: f32,
    ) {
        let Ok(layout) = tree.layout(tid) else { return };
        let mut rect = Rect {
            x: px + layout.location.x,
            y: py + layout.location.y,
            w: layout.size.width,
            h: layout.size.height,
        };
        if let Some([ax, ay]) = nodes[i].desc.anchor {
            rect.x = f32::mul_add(ax, -rect.w, rect.x);
            rect.y = f32::mul_add(ay, -rect.h, rect.y);
        }
        nodes[i].rect = rect;
        // Content extent such that `content - size` is the maximum scroll offset.
        nodes[i].content = (
            rect.w + layout.scroll_width(),
            rect.h + layout.scroll_height(),
        );
        #[allow(clippy::needless_collect)]
        let live_children: Vec<usize> = nodes[i]
            .children
            .iter()
            .copied()
            .filter(|&c| !nodes[c].in_ghost)
            .collect();
        let tchildren = tree.children(tid).unwrap_or_default();
        for (c, tc) in live_children.into_iter().zip(tchildren) {
            Self::assign_rects(tree, nodes, tc, c, rect.x, rect.y);
        }
    }
}

fn layout_style(node: &Node, root_size: Option<(f32, f32)>) -> tf::Style {
    let mut style = taffy_style(&node.desc.style);
    if node.desc.kind() == NodeKind::Slider && node.desc.style.height.is_none() {
        style.size.height =
            tf::Dimension::length(node.desc.style.thumb_size.unwrap_or(SLIDER_HEIGHT));
    }
    if let Some((width, height)) = root_size {
        style.position = tf::Position::Relative;
        style.inset = tf::Rect::auto();
        style.size = tf::Size {
            width: tf::Dimension::length(width),
            height: tf::Dimension::length(height),
        };
    }
    if node.tooltip_hidden {
        style.display = tf::Display::None;
    }
    style
}

fn measurement_changed(old: &Node, new: &Node) -> bool {
    match new.desc.kind() {
        NodeKind::Text => {
            old.text_style != new.text_style
                || old
                    .spans
                    .as_ref()
                    .zip(new.spans.as_ref())
                    .is_none_or(|(a, b)| {
                        a.len() != b.len()
                            || a.iter().zip(b.iter()).any(|(a, b)| {
                                (&a.text, a.b, a.i, &a.font, a.size)
                                    != (&b.text, b.b, b.i, &b.font, b.size)
                            })
                    })
        }
        NodeKind::Input => {
            old.text_style != new.text_style
                || old.desc.string_value() != new.desc.string_value()
                || old.desc.placeholder != new.desc.placeholder
        }
        NodeKind::Image | NodeKind::Video => old.desc.src != new.desc.src,
        NodeKind::Box | NodeKind::Slider => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn measure(
    node: &Node,
    known: tf::Size<Option<f32>>,
    available: tf::Size<tf::AvailableSpace>,
    scale: f32,
    text: &mut TextSystem,
    assets: &mut Assets,
    reveals: &HashMap<String, Reveal>,
    videos: &HashMap<String, VideoPlayer>,
    now: Instant,
) -> tf::Size<f32> {
    if let tf::Size {
        width: Some(width),
        height: Some(height),
    } = known
    {
        return tf::Size { width, height };
    }
    let natural = |iw: f32, ih: f32| match (known.width, known.height) {
        (Some(w), None) if iw > 0.0 => tf::Size {
            width: w,
            height: w * ih / iw,
        },
        (None, Some(h)) if ih > 0.0 => tf::Size {
            width: h * iw / ih,
            height: h,
        },
        _ => tf::Size {
            width: iw,
            height: ih,
        },
    };
    match node.desc.kind() {
        NodeKind::Text | NodeKind::Input => {
            let limit = known.width.or(match available.width {
                tf::AvailableSpace::Definite(w) => Some(w),
                tf::AvailableSpace::MinContent => Some(0.0),
                tf::AvailableSpace::MaxContent => None,
            });
            let (w, h) = if let Some(spans) = &node.spans {
                let revealed = reveals.get(&node.id).map_or(usize::MAX, |r| r.shown(now));
                text.prepare(
                    &node.id,
                    spans,
                    &node.text_style,
                    scale,
                    limit,
                    revealed,
                    1.0,
                    None,
                )
            } else {
                let content = input_display(node).0;
                let spans = [SpanDesc {
                    text: content,
                    ..Default::default()
                }];
                let (w, h) = text.prepare(
                    &node.id,
                    &spans,
                    &node.text_style,
                    scale,
                    None,
                    usize::MAX,
                    1.0,
                    None,
                );
                let line = node.text_style.font_size * node.text_style.line_height * scale;
                (w, h.max(line))
            };
            // Round up so the final layout width never wraps differently than the measurement.
            tf::Size {
                width: known.width.unwrap_or_else(|| (w / scale).ceil() + 1.0),
                height: known.height.unwrap_or(h / scale),
            }
        }
        NodeKind::Image => {
            let Some((iw, ih)) = node.desc.src.as_deref().and_then(|s| assets.image_size(s)) else {
                return tf::Size::ZERO;
            };
            natural(iw.as_(), ih.as_())
        }
        NodeKind::Video => match videos.get(&node.id).and_then(VideoPlayer::size) {
            Some((w, h)) => natural(w.as_(), h.as_()),
            None => tf::Size::ZERO,
        },
        NodeKind::Slider => tf::Size {
            width: known.width.unwrap_or(200.0),
            height: known.height.unwrap_or(SLIDER_HEIGHT),
        },
        NodeKind::Box => tf::Size::ZERO,
    }
}

pub(super) fn input_display(node: &Node) -> (String, bool) {
    let value = node.desc.string_value();
    if value.is_empty() {
        (node.desc.placeholder.clone().unwrap_or_default(), true)
    } else {
        (value.to_owned(), false)
    }
}

const fn dimension(d: Option<Dim>) -> tf::Dimension {
    match d {
        Some(Dim::Px(v)) => tf::Dimension::length(v),
        Some(Dim::Percent(v)) => tf::Dimension::percent(v),
        Some(Dim::Auto) | None => tf::Dimension::auto(),
    }
}

const fn inset(d: Option<Dim>) -> tf::LengthPercentageAuto {
    match d {
        Some(Dim::Px(v)) => tf::LengthPercentageAuto::length(v),
        Some(Dim::Percent(v)) => tf::LengthPercentageAuto::percent(v),
        Some(Dim::Auto) | None => tf::LengthPercentageAuto::auto(),
    }
}

fn edges_lp(e: Option<Edges>) -> tf::Rect<tf::LengthPercentage> {
    let [top, right, bottom, left] = e.map_or([0.0; 4], Edges::trbl);
    tf::Rect {
        left: tf::LengthPercentage::length(left),
        right: tf::LengthPercentage::length(right),
        top: tf::LengthPercentage::length(top),
        bottom: tf::LengthPercentage::length(bottom),
    }
}

fn edges_lpa(e: Option<Edges>) -> tf::Rect<tf::LengthPercentageAuto> {
    let [top, right, bottom, left] = e.map_or([0.0; 4], Edges::trbl);
    tf::Rect {
        left: tf::LengthPercentageAuto::length(left),
        right: tf::LengthPercentageAuto::length(right),
        top: tf::LengthPercentageAuto::length(top),
        bottom: tf::LengthPercentageAuto::length(bottom),
    }
}

fn align_items(a: Option<Align>) -> Option<tf::AlignItems> {
    Some(match a? {
        Align::Start => tf::AlignItems::FLEX_START,
        Align::End => tf::AlignItems::FLEX_END,
        Align::Center => tf::AlignItems::CENTER,
        Align::Stretch => tf::AlignItems::STRETCH,
        Align::Baseline => tf::AlignItems::BASELINE,
        _ => return None,
    })
}

fn justify_content(a: Option<Align>) -> Option<tf::JustifyContent> {
    Some(match a? {
        Align::Start => tf::JustifyContent::FLEX_START,
        Align::End => tf::JustifyContent::FLEX_END,
        Align::Center => tf::JustifyContent::CENTER,
        Align::Stretch => tf::JustifyContent::STRETCH,
        Align::SpaceBetween => tf::JustifyContent::SPACE_BETWEEN,
        Align::SpaceAround => tf::JustifyContent::SPACE_AROUND,
        Align::SpaceEvenly => tf::JustifyContent::SPACE_EVENLY,
        Align::Baseline => return None,
    })
}

fn tracks(count: Option<u16>) -> Vec<tf::GridTemplateComponent<String>> {
    count.map_or_else(Vec::new, |n| {
        (0..n.max(1)).map(|_| tf::style_helpers::fr(1.0)).collect()
    })
}

fn taffy_style(s: &Style) -> tf::Style {
    let border = s.border_width.unwrap_or(0.0);
    let grid =
        s.display == Some(Display::Grid) || s.grid_columns.is_some() || s.grid_rows.is_some();
    let overflow = match s.overflow {
        Some(Overflow::Hidden | Overflow::Scroll) => tf::Overflow::Hidden,
        _ => tf::Overflow::Visible,
    };
    let gap = tf::LengthPercentage::length(s.gap.unwrap_or(0.0));
    tf::Style {
        display: match s.display {
            Some(Display::None) => tf::Display::None,
            _ if grid => tf::Display::Grid,
            _ => tf::Display::Flex,
        },
        position: if s.position == Some(Position::Absolute) {
            tf::Position::Absolute
        } else {
            tf::Position::Relative
        },
        overflow: tf::Point {
            x: overflow,
            y: overflow,
        },
        inset: tf::Rect {
            left: inset(s.left),
            right: inset(s.right),
            top: inset(s.top),
            bottom: inset(s.bottom),
        },
        size: tf::Size {
            width: dimension(s.width),
            height: dimension(s.height),
        },
        min_size: tf::Size {
            width: inset(s.min_width),
            height: inset(s.min_height),
        },
        max_size: tf::Size {
            width: inset(s.max_width),
            height: inset(s.max_height),
        },
        padding: edges_lp(s.padding),
        margin: edges_lpa(s.margin),
        border: edges_lp(Some(Edges::All(border))),
        gap: tf::Size {
            width: gap,
            height: gap,
        },
        flex_direction: match s.flex_direction {
            Some(FlexDirection::Column) => tf::FlexDirection::Column,
            Some(FlexDirection::RowReverse) => tf::FlexDirection::RowReverse,
            Some(FlexDirection::ColumnReverse) => tf::FlexDirection::ColumnReverse,
            Some(FlexDirection::Row) | None => tf::FlexDirection::Row,
        },
        flex_wrap: if s.flex_wrap == Some(FlexWrap::Wrap) {
            tf::FlexWrap::Wrap
        } else {
            tf::FlexWrap::NoWrap
        },
        flex_grow: s.flex_grow.unwrap_or(0.0),
        flex_shrink: s.flex_shrink.unwrap_or(1.0),
        justify_content: justify_content(s.justify_content),
        align_items: align_items(s.align_items),
        align_self: align_items(s.align_self),
        grid_template_columns: tracks(s.grid_columns),
        grid_template_rows: tracks(s.grid_rows),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests;
