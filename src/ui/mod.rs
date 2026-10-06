//! Retained UI: receives element trees from JS, diffs them against the
//! previous tree (enter/exit/move animations, transforms, typewriter state),
//! lays them out with taffy, and produces draw lists and hit tests.

pub mod desc;
mod draw;
mod input;
mod layout;
pub mod reveal;
pub mod text;
pub mod transform;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use crate::assets::Assets;
use crate::ui::desc::FontWeight;
use crate::video::VideoPlayer;
use desc::{AnimDesc, Color, Ease, NodeDesc, NodeKind, SpanDesc, Style};
use reveal::Reveal;
use text::{TextStyle, TextSystem};

pub use input::{InputEvent, Nav};

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }

    pub fn intersect(&self, o: &Self) -> Self {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = (self.x + self.w).min(o.x + o.w);
        let y1 = (self.y + self.h).min(o.y + o.h);
        Self {
            x: x0,
            y: y0,
            w: (x1 - x0).max(0.0),
            h: (y1 - y0).max(0.0),
        }
    }

    fn center(&self) -> (f32, f32) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }
}

/// A running enter/exit/move animation.
#[derive(Clone)]
struct Timed {
    start: Instant,
    spec: AnimDesc,
}

impl Timed {
    fn progress(&self, now: Instant, default: Ease) -> f32 {
        if self.spec.dur <= 0.0 {
            return 1.0;
        }
        let t = now.saturating_duration_since(self.start).as_secs_f32() / self.spec.dur;
        self.spec.ease_or(default).apply(t)
    }

    fn finished(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.start).as_secs_f32() >= self.spec.dur
    }
}

struct Move {
    start: Instant,
    spec: AnimDesc,
    dx: f32,
    dy: f32,
}

#[derive(Clone)]
struct Node {
    id: String,
    desc: NodeDesc,
    /// Rich text content (plain `text` is converted to a single span).
    spans: Option<Arc<Vec<SpanDesc>>>,
    parent: Option<usize>,
    children: Vec<usize>,
    text_style: TextStyle,
    /// Layout rectangle in virtual units, absolute (before scrolling).
    rect: Rect,
    /// Size of the laid-out content, for scroll containers.
    content: (f32, f32),
    /// Set on the root of a subtree that is playing its exit animation.
    ghost: Option<Timed>,
    /// Part of an exiting subtree: frozen layout, not interactive.
    in_ghost: bool,
}

#[derive(Debug, Clone)]
pub struct ImageRef {
    pub src: String,
    /// u0, v0, u1, v1
    pub uv: [f32; 4],
    /// Decoded video frame: (frame serial, pixels).
    pub frame: Option<(u64, Arc<image::RgbaImage>)>,
}

/// Screen-space transition mask applied to a quad.
#[derive(Debug, Clone)]
pub struct MaskDraw {
    /// 1 = image, 2..=5 = wipe left/right/up/down, 6 = pixellate.
    pub kind: u32,
    pub progress: f32,
    /// Ramp width (image/wipe) or maximum block size in pixels (pixellate).
    pub param: f32,
    /// Hide instead of reveal (exiting elements).
    pub invert: bool,
    pub src: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Quad {
    /// Unrotated rectangle; rotation is around its center.
    pub rect: Rect,
    pub rotation: f32,
    pub color: Color,
    pub radius: f32,
    pub border_width: f32,
    pub border_color: Color,
    pub image: Option<ImageRef>,
    pub clip: Rect,
    pub mask: Option<MaskDraw>,
}

#[derive(Debug, Clone)]
pub struct TextDraw {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub color: Color,
    pub clip: Rect,
}

#[derive(Debug, Clone)]
pub enum DrawItem {
    Quad(Quad),
    Text(TextDraw),
}

pub struct Ui {
    nodes: Vec<Node>,
    index: HashMap<String, usize>,
    enters: HashMap<String, Timed>,
    moves: HashMap<String, Move>,
    pending_moves: HashMap<String, (Rect, AnimDesc)>,
    transforms: HashMap<String, (Instant, desc::TransformDesc)>,
    reveals: HashMap<String, Reveal>,
    videos: HashMap<String, VideoPlayer>,
    scroll: HashMap<String, (f32, f32)>,
    pub text: TextSystem,
    default_font: String,
    virtual_size: (f32, f32),
    scale: f32,
    offset: (f32, f32),
    layout_dirty: bool,
    hovered: HashSet<usize>,
    cursor: Option<(f32, f32)>,
    focused: Option<String>,
    dragging: Option<String>,
    tooltip: Option<String>,
    /// Interactive nodes in draw order with their on-screen (clipped) bounds, from the last frame.
    hit_order: Vec<(usize, Rect)>,
    was_revealing: bool,
}

impl Ui {
    pub fn new(text: TextSystem) -> Self {
        Self {
            nodes: Vec::new(),
            index: HashMap::new(),
            enters: HashMap::new(),
            moves: HashMap::new(),
            pending_moves: HashMap::new(),
            transforms: HashMap::new(),
            reveals: HashMap::new(),
            videos: HashMap::new(),
            scroll: HashMap::new(),
            text,
            default_font: String::new(),
            virtual_size: (1280.0, 720.0),
            scale: 1.0,
            offset: (0.0, 0.0),
            layout_dirty: true,
            hovered: HashSet::new(),
            cursor: None,
            focused: None,
            dragging: None,
            tooltip: None,
            hit_order: Vec::new(),
            was_revealing: false,
        }
    }

    pub fn set_config(&mut self, width: f32, height: f32, font: &str) {
        let new_size = (width.max(1.0), height.max(1.0));

        if self.virtual_size != new_size || self.default_font != font {
            self.virtual_size = new_size;

            if self.default_font != font {
                font.clone_into(&mut self.default_font);
            }

            self.layout_dirty = true;
        }
    }

    /// Updates the letterboxed mapping from virtual units to the physical surface.
    // Any change to the mapping must invalidate layout, even below an epsilon.
    #[allow(clippy::float_cmp)]
    pub fn set_surface_size(&mut self, width: f32, height: f32) {
        let (vw, vh) = self.virtual_size;
        let scale = (width / vw).min(height / vh).max(0.01);
        let offset = (
            (f32::mul_add(vw, -scale, width) / 2.0).floor(),
            (f32::mul_add(vh, -scale, height) / 2.0).floor(),
        );
        if scale != self.scale || offset != self.offset {
            self.scale = scale;
            self.offset = offset;
            self.layout_dirty = true;
        }
    }

    /// The letterboxed game area in physical pixels.
    pub fn viewport(&self) -> Rect {
        Rect {
            x: self.offset.0,
            y: self.offset.1,
            w: self.virtual_size.0 * self.scale,
            h: self.virtual_size.1 * self.scale,
        }
    }

    /// Image sources referenced by the tree (for preloading before commit).
    pub fn collect_images(tree: &NodeDesc, out: &mut Vec<String>) {
        out.extend(
            tree.src
                .iter()
                .filter(|_| tree.kind() == NodeKind::Image)
                .cloned(),
        );
        out.extend(tree.hover_src.iter().cloned());
        for anim in [&tree.enter, &tree.exit].into_iter().flatten() {
            if let Some(desc::MaskDesc::Image { src, .. }) = &anim.mask {
                out.push(src.clone());
            }
        }
        for child in &tree.children {
            Self::collect_images(child, out);
        }
    }

    // -----------------------------------------------------------------------
    // Commit
    // -----------------------------------------------------------------------

    /// Replaces the UI tree, starting enter/exit/move animations for changed keyed elements.
    pub fn commit(
        &mut self,
        tree: NodeDesc,
        instant: bool,
        exits: &HashMap<String, Option<AnimDesc>>,
        assets: &Assets,
        now: Instant,
    ) {
        let old_nodes = std::mem::take(&mut self.nodes);
        let old_index = std::mem::take(&mut self.index);
        let root_style = TextStyle {
            font_size: 24.0,
            line_height: 1.3,
            family: self.default_font.clone(),
            weight: 400,
            italic: false,
            align: None,
        };
        self.build(tree, None, "", 0, &root_style);
        let built = self.nodes.len();
        self.keep_exits(&old_nodes, &old_index, instant, exits, now);

        // Enter animations for new keyed elements; moves for keyed elements that existed.
        if instant {
            self.enters.clear();
            self.moves.clear();
        }
        self.pending_moves.clear();
        for node in &self.nodes[..built] {
            match old_index.get(&node.id) {
                None => {
                    if let (Some(spec), false) = (&node.desc.enter, instant) {
                        self.enters.insert(
                            node.id.clone(),
                            Timed {
                                start: now,
                                spec: spec.clone(),
                            },
                        );
                    }
                }
                Some(&oi) => {
                    if let (Some(spec), false) = (&node.desc.r#move, instant) {
                        let old = old_nodes[oi].rect;
                        let (dx, dy) = self.moves.get(&node.id).map_or((0.0, 0.0), |m| {
                            let e = Timed {
                                start: m.start,
                                spec: m.spec.clone(),
                            }
                            .progress(now, Ease::EaseInOut);
                            (m.dx * (1.0 - e), m.dy * (1.0 - e))
                        });
                        let visual = Rect {
                            x: old.x + dx,
                            y: old.y + dy,
                            ..old
                        };
                        self.pending_moves
                            .insert(node.id.clone(), (visual, spec.clone()));
                    }
                }
            }
        }
        self.enters.retain(|id, _| self.index.contains_key(id));
        self.moves.retain(|id, _| self.index.contains_key(id));

        self.sync_content(built, assets, now, instant);

        self.scroll.retain(|id, _| self.index.contains_key(id));
        if self
            .focused
            .as_ref()
            .is_some_and(|f| !self.index.contains_key(f))
        {
            self.focused = None;
        }
        if let Some(node) = self.nodes[..built].iter().find(|n| n.desc.autofocus)
            && !old_index.contains_key(&node.id)
        {
            self.focused = Some(node.id.clone());
        }
        if self
            .dragging
            .as_ref()
            .is_some_and(|d| !self.index.contains_key(d))
        {
            self.dragging = None;
        }

        let live: HashSet<&str> = self.nodes.iter().map(|n| n.id.as_str()).collect();
        self.text.retain(|id| live.contains(id));
        self.hovered.clear();
        self.hit_order.clear();
        self.layout_dirty = true;
    }

    /// Keeps removed elements at their old z position until their exit animation finishes.
    fn keep_exits(
        &mut self,
        old_nodes: &[Node],
        old_index: &HashMap<String, usize>,
        instant: bool,
        exits: &HashMap<String, Option<AnimDesc>>,
        now: Instant,
    ) {
        let built = self.nodes.len();
        for i in 0..built {
            let Some(&oi) = old_index.get(&self.nodes[i].id) else {
                continue;
            };
            let mut children = self.nodes[i].children.clone();
            let mut pos = 0;
            for &oc in &old_nodes[oi].children {
                let old = &old_nodes[oc];
                if let Some(p) = children.iter().position(|&c| self.nodes[c].id == old.id) {
                    pos = p + 1;
                    continue;
                }
                let ghost = match &old.ghost {
                    Some(g) if !instant => Some(g.clone()),
                    Some(_) => None,
                    None if instant || old.in_ghost => None,
                    None => {
                        let spec = old
                            .desc
                            .key
                            .as_deref()
                            .and_then(|k| exits.get(k))
                            .map_or_else(|| old.desc.exit.clone(), Clone::clone);
                        spec.map(|spec| Timed { start: now, spec })
                    }
                };
                if let Some(ghost) = ghost.filter(|g| !g.finished(now)) {
                    let gi = self.copy_ghost(old_nodes, oc, i, Some(ghost));
                    children.insert(pos, gi);
                    pos += 1;
                }
            }
            self.nodes[i].children = children;
        }
    }

    fn sync_content(&mut self, built: usize, assets: &Assets, now: Instant, instant: bool) {
        // Transforms restart when their program changes.
        let mut transforms = HashMap::new();
        for node in &self.nodes[..built] {
            let Some(program) = &node.desc.transform else {
                continue;
            };
            let entry = match self.transforms.remove(&node.id) {
                Some((start, old)) if old == *program => (start, old),
                _ => (now, program.clone()),
            };
            transforms.insert(node.id.clone(), entry);
        }
        self.transforms = transforms;

        // Typewriter state survives re-renders while the text is unchanged.
        let mut reveals = HashMap::new();
        for node in &self.nodes[..built] {
            let (Some(spans), Some(cps)) = (&node.spans, node.desc.cps) else {
                continue;
            };
            let reveal = match self.reveals.remove(&node.id) {
                Some(r) if r.spans == **spans => r,
                _ => Reveal::new(spans.to_vec(), cps, now, instant),
            };
            reveals.insert(node.id.clone(), reveal);
        }
        self.reveals = reveals;

        // Videos keep playing across re-renders of the same element and source.
        let mut videos = HashMap::new();
        for node in &self.nodes[..built] {
            if node.desc.kind() != NodeKind::Video {
                continue;
            }
            let Some(src) = &node.desc.src else { continue };
            let player = match self.videos.remove(&node.id) {
                Some(p) if p.src() == src => p,
                _ => match assets.resolve(src) {
                    Some(path) => VideoPlayer::open(src, &path, node.desc.r#loop, now),
                    None => continue,
                },
            };
            videos.insert(node.id.clone(), player);
        }
        self.videos = videos;
    }

    fn build(
        &mut self,
        mut desc: NodeDesc,
        parent: Option<usize>,
        parent_id: &str,
        child_index: usize,
        inherited: &TextStyle,
    ) -> usize {
        let id = desc.key.as_ref().map_or_else(
            || format!("{parent_id}/#{child_index}"),
            |key| format!("{parent_id}/{key}"),
        );
        let text_style = inherit_text(inherited, &desc.style);
        let children = std::mem::take(&mut desc.children);
        let spans = match desc.kind() {
            NodeKind::Text => Some(Arc::new(desc.spans.take().unwrap_or_else(|| {
                vec![SpanDesc {
                    text: desc.text.take().unwrap_or_default(),
                    ..Default::default()
                }]
            }))),
            _ => None,
        };
        let idx = self.nodes.len();
        self.nodes.push(Node {
            id: id.clone(),
            desc,
            spans,
            parent,
            children: Vec::new(),
            text_style: text_style.clone(),
            rect: Rect::default(),
            content: (0.0, 0.0),
            ghost: None,
            in_ghost: false,
        });
        self.index.insert(id.clone(), idx);
        let kids = children
            .into_iter()
            .enumerate()
            .map(|(i, child)| self.build(child, Some(idx), &id, i, &text_style))
            .collect();
        self.nodes[idx].children = kids;
        idx
    }

    fn copy_ghost(
        &mut self,
        old: &[Node],
        oi: usize,
        parent: usize,
        ghost: Option<Timed>,
    ) -> usize {
        let idx = self.nodes.len();
        let mut node = old[oi].clone();
        node.parent = Some(parent);
        node.in_ghost = true;
        node.ghost = ghost.or(node.ghost);
        node.children = Vec::new();
        self.nodes.push(node);
        let kids = old[oi]
            .children
            .iter()
            .map(|&c| self.copy_ghost(old, c, idx, None))
            .collect();
        self.nodes[idx].children = kids;
        idx
    }

    // -----------------------------------------------------------------------
    // State queries
    // -----------------------------------------------------------------------

    /// True while any animation, typewriter effect or video is running.
    pub fn is_animating(&self, now: Instant) -> bool {
        self.enters.values().any(|a| !a.finished(now))
            || self
                .moves
                .values()
                .any(|m| now.saturating_duration_since(m.start).as_secs_f32() < m.spec.dur)
            || !self.pending_moves.is_empty()
            || self
                .nodes
                .iter()
                .any(|n| n.ghost.as_ref().is_some_and(|g| !g.finished(now)))
            || self.transforms.values().any(|(start, program)| {
                transform::duration(&program.steps)
                    .is_none_or(|d| now.saturating_duration_since(*start).as_secs_f32() < d)
            })
            || self.videos.values().any(|v| !v.ended())
            || self.reveals.values().any(|r| r.is_typing(now))
    }

    /// True while enter, exit or move transitions are playing.
    pub fn is_transitioning(&self, now: Instant) -> bool {
        self.enters.values().any(|a| !a.finished(now))
            || self
                .moves
                .values()
                .any(|m| now.saturating_duration_since(m.start).as_secs_f32() < m.spec.dur)
            || self
                .nodes
                .iter()
                .any(|n| n.ghost.as_ref().is_some_and(|g| !g.finished(now)))
    }

    pub fn is_revealing(&self, now: Instant) -> bool {
        self.reveals.values().any(|r| r.is_revealing(now))
    }

    /// Shows text up to the next click-wait, or continues after one.
    pub fn reveal_skip(&mut self, now: Instant) {
        for reveal in self.reveals.values_mut() {
            reveal.skip(now);
        }
    }

    /// Returns true once when all typewriter text has finished revealing.
    pub fn take_revealed_event(&mut self, now: Instant) -> bool {
        let revealing = self.is_revealing(now);
        let fired = self.was_revealing && !revealing;
        self.was_revealing = revealing;
        fired
    }

    /// Handlers of videos that finished since the last call.
    pub fn take_ended_videos(&mut self) -> Vec<u32> {
        let mut ended = Vec::new();
        for (id, player) in &mut self.videos {
            if player.take_ended()
                && let Some(&i) = self.index.get(id)
                && let Some(h) = self.nodes[i].desc.on_end
            {
                ended.push(h);
            }
        }
        ended
    }

    /// Sources of the videos currently on screen (their audio is played by the engine).
    pub fn video_sources(&self) -> impl Iterator<Item = (&str, bool)> {
        self.videos.values().map(|v| (v.src(), v.looping()))
    }

    /// Drops finished animations so the tree stays small.
    pub fn prune(&mut self, now: Instant) {
        self.enters.retain(|_, a| !a.finished(now));
        self.moves
            .retain(|_, m| now.saturating_duration_since(m.start).as_secs_f32() < m.spec.dur);
    }
}

fn inherit_text(parent: &TextStyle, style: &Style) -> TextStyle {
    TextStyle {
        font_size: style.font_size.unwrap_or(parent.font_size),
        line_height: style.line_height.unwrap_or(parent.line_height),
        family: style
            .font_family
            .clone()
            .unwrap_or_else(|| parent.family.clone()),
        weight: style.font_weight.map_or(parent.weight, FontWeight::value),
        italic: style.italic.unwrap_or(parent.italic),
        align: style.text_align.or(parent.align),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use desc::Dim;
    use std::path::Path;

    #[test]
    fn input_between_commit_and_draw_ignores_previous_tree() {
        let game_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("game");
        let mut assets = Assets::new(game_dir.clone());
        let mut ui = Ui::new(TextSystem::new(&game_dir));
        let now = Instant::now();
        let exits = HashMap::new();
        let button = NodeDesc {
            on_click: Some(1),
            style: Style {
                width: Some(Dim::Px(100.0)),
                height: Some(Dim::Px(100.0)),
                ..Default::default()
            },
            ..Default::default()
        };

        // Cover both an out-of-bounds old index and an index reused by a new node.
        for child_count in [0, 1] {
            ui.commit(
                NodeDesc {
                    children: vec![button.clone()],
                    ..Default::default()
                },
                true,
                &exits,
                &assets,
                now,
            );
            ui.draw(&mut assets, now);
            ui.pointer_moved(Some((10.0, 10.0)));
            assert_eq!(ui.click_target(), Some(1));

            ui.commit(
                NodeDesc {
                    on_click: Some(2),
                    children: vec![
                        NodeDesc {
                            on_click: Some(3),
                            ..button.clone()
                        };
                        child_count
                    ],
                    ..Default::default()
                },
                true,
                &exits,
                &assets,
                now,
            );

            ui.pointer_moved(Some((11.0, 11.0)));
            assert_eq!(ui.click_target(), None);
            assert_eq!(ui.mouse_down(), []);
            assert!(!ui.scroll_at(1.0));
            assert_eq!(ui.navigate(Nav::Down), (false, Vec::new()));

            ui.draw(&mut assets, now);
            ui.refresh_hover();
            let expected = if child_count == 0 { 2 } else { 3 };
            assert_eq!(ui.click_target(), Some(expected));
            assert_eq!(ui.mouse_down(), vec![InputEvent::Click { h: expected }]);
        }
    }
}
