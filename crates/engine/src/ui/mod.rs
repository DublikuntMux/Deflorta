//! Retained UI: receives element trees from JS, diffs them against the
//! previous tree (enter/exit/move animations, transforms, typewriter state),
//! lays them out with taffy, and produces draw lists and hit tests.

mod accessibility;
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
use crate::script::Handler;
use crate::ui::desc::FontWeight;
use crate::util::time::elapsed_secs;
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
        let t = elapsed_secs(self.start, now) / self.spec.dur;
        self.spec.ease_or(default).apply(t)
    }

    fn finished(&self, now: Instant) -> bool {
        elapsed_secs(self.start, now) >= self.spec.dur
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
    /// Native tooltip text is absent, so this text node occupies no space.
    tooltip_hidden: bool,
    parent: Option<usize>,
    children: Vec<usize>,
    text_style: TextStyle,
    /// Layout rectangle in virtual units, absolute (before scrolling).
    rect: Rect,
    /// Size of the laid-out content, for scroll containers.
    content: (f32, f32),
    /// Retained layout identity; exiting nodes use their frozen rectangles.
    layout_id: Option<taffy::NodeId>,
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
    layout_tree: taffy::TaffyTree<usize>,
    hovered: HashSet<usize>,
    cursor: Option<(f32, f32)>,
    focused: Option<String>,
    dragging: Option<String>,
    tooltip: Option<String>,
    /// Interactive nodes in draw order with their on-screen (clipped) bounds, from the last frame.
    hit_order: Vec<(usize, Rect)>,
    was_revealing: bool,
    accessibility: accessibility::Accessibility,
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
            layout_tree: taffy::TaffyTree::new(),
            hovered: HashSet::new(),
            cursor: None,
            focused: None,
            dragging: None,
            tooltip: None,
            hit_order: Vec::new(),
            was_revealing: false,
            accessibility: accessibility::Accessibility::default(),
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
            self.invalidate_measurements();
        }
    }

    #[cfg(feature = "dev-console")]
    pub fn diagnostic_counts(&self) -> (usize, usize, usize) {
        (
            self.nodes.len(),
            self.text.buffer_count(),
            self.videos.len(),
        )
    }

    #[cfg(feature = "dev-console")]
    pub fn loaded_assets(&self) -> Vec<crate::dev_console::diagnostics::LoadedAsset> {
        let mut assets = self
            .videos
            .values()
            .map(|video| crate::dev_console::diagnostics::LoadedAsset {
                kind: "Video",
                source: video.src().into(),
                state: if video.ended() { "Ended" } else { "Streaming" }.into(),
                detail: video
                    .size()
                    .map_or_else(|| "Opening".into(), |(w, h)| format!("{w}×{h}")),
                bytes: None,
            })
            .collect::<Vec<_>>();
        assets.extend(self.text.font_system.db().faces().map(|face| {
            crate::dev_console::diagnostics::LoadedAsset {
                kind: "Font",
                source: face.post_script_name.clone(),
                state: "Loaded face".into(),
                detail: format!(
                    "{} · weight {} · {:?}",
                    face.families
                        .iter()
                        .map(|(name, _)| name.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                    face.weight.0,
                    face.style
                ),
                bytes: None,
            }
        }));
        assets
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
            if scale != self.scale {
                self.invalidate_measurements();
            }
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

    /// Sources the current tree may draw, including hover images and live exit masks.
    pub fn retained_assets(&self, now: Instant) -> HashSet<String> {
        let mut sources = Vec::new();
        for (index, node) in self.nodes.iter().enumerate() {
            if self.ancestors(index).any(|i| {
                self.nodes[i]
                    .ghost
                    .as_ref()
                    .is_some_and(|ghost| ghost.finished(now))
            }) {
                continue;
            }
            Self::collect_images(&node.desc, &mut sources);
            for animation in [node.ghost.as_ref(), self.enters.get(&node.id)]
                .into_iter()
                .flatten()
            {
                if let Some(desc::MaskDesc::Image { src, .. }) = &animation.spec.mask {
                    sources.push(src.clone());
                }
            }
            if self.videos.contains_key(&node.id) {
                sources.push(format!("video:{}", node.id));
            }
        }
        sources.into_iter().collect()
    }

    /// Stops video decoders by source or by their GPU asset ID.
    #[cfg(feature = "dev-console")]
    pub fn unload_video(&mut self, source: &str) -> Vec<(String, String)> {
        let video_source = self.videos.iter().find_map(|(id, video)| {
            (format!("video:{id}") == source).then(|| video.src().to_owned())
        });
        let source = video_source.as_deref().unwrap_or(source);
        let mut released = Vec::new();
        self.videos.retain(|id, video| {
            let texture = format!("video:{id}");
            if video.src() != source {
                return true;
            }
            released.push((texture, video.src().to_owned()));
            false
        });
        released
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
        self.reconcile_layout(&old_nodes, &old_index);

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
        self.layout_dirty |= !self.pending_moves.is_empty();
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
                _ => match assets.game_path(src) {
                    Some(path) => {
                        VideoPlayer::open(src, assets.files(), &path, node.desc.r#loop, now)
                    }
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
        if desc.kind() == NodeKind::Text && desc.tooltip_text {
            desc.cps = None;
        }
        let tooltip_hidden = desc.kind() == NodeKind::Text
            && desc.tooltip_text
            && self.tooltip.as_deref().is_none_or(str::is_empty);
        let spans = match desc.kind() {
            NodeKind::Text if desc.tooltip_text => Some(Arc::new(vec![SpanDesc {
                text: self.tooltip.clone().unwrap_or_default(),
                ..Default::default()
            }])),
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
            tooltip_hidden,
            parent,
            children: Vec::new(),
            text_style: text_style.clone(),
            rect: Rect::default(),
            content: (0.0, 0.0),
            layout_id: None,
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
        node.layout_id = None;
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
                .any(|m| elapsed_secs(m.start, now) < m.spec.dur)
            || !self.pending_moves.is_empty()
            || self
                .nodes
                .iter()
                .any(|n| n.ghost.as_ref().is_some_and(|g| !g.finished(now)))
            || self.transforms.values().any(|(start, program)| {
                transform::duration(&program.steps).is_none_or(|d| elapsed_secs(*start, now) < d)
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
                .any(|m| elapsed_secs(m.start, now) < m.spec.dur)
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
    pub fn take_ended_videos(&mut self) -> Vec<Handler> {
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
            .retain(|_, m| elapsed_secs(m.start, now) < m.spec.dur);
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

    #[test]
    fn retained_assets_preserve_hover_and_exit_masks_until_the_ghost_finishes() {
        let files = crate::GameFiles::open(&crate::workspace_dir().join("game")).unwrap();
        let mut ui = Ui::new(TextSystem::new(&files));
        let assets = Assets::new(files);
        let now = Instant::now();
        let old: NodeDesc = serde_json::from_value(serde_json::json!({
            "children": [{
                "key": "old", "t": "image", "src": "old.png", "hoverSrc": "old-hover.png",
                "children": [{ "t": "image", "src": "ghost-child.png" }]
            }]
        }))
        .unwrap();
        ui.commit(old, true, &HashMap::new(), &assets, now);
        let transition: AnimDesc = serde_json::from_value(serde_json::json!({
            "dur": 2.0, "mask": {"kind": "image", "src": "exit-mask.png"}
        }))
        .unwrap();
        let new: NodeDesc = serde_json::from_value(serde_json::json!({
            "children": [{ "t": "image", "src": "current.png", "hoverSrc": "hover.png" }]
        }))
        .unwrap();
        ui.commit(
            new,
            false,
            &HashMap::from([("old".into(), Some(transition))]),
            &assets,
            now,
        );
        assert_eq!(
            ui.retained_assets(now),
            HashSet::from(
                [
                    "old.png",
                    "old-hover.png",
                    "ghost-child.png",
                    "exit-mask.png",
                    "current.png",
                    "hover.png"
                ]
                .map(str::to_owned)
            )
        );
        assert_eq!(
            ui.retained_assets(now + std::time::Duration::from_secs(2)),
            HashSet::from(["current.png".to_owned(), "hover.png".to_owned()])
        );
    }

    #[test]
    fn input_between_commit_and_draw_ignores_previous_tree() {
        let files = crate::GameFiles::open(&crate::workspace_dir().join("game")).unwrap();
        let mut ui = Ui::new(TextSystem::new(&files));
        let mut assets = Assets::new(files);
        let now = Instant::now();
        let exits = HashMap::new();
        let handler = |index| Handler {
            generation: 1,
            index,
        };
        let button = NodeDesc {
            on_click: Some(handler(1)),
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
            assert_eq!(ui.click_target(), Some(handler(1)));

            ui.commit(
                NodeDesc {
                    on_click: Some(handler(2)),
                    children: vec![
                        NodeDesc {
                            on_click: Some(handler(3)),
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
            let expected = handler(if child_count == 0 { 2 } else { 3 });
            assert_eq!(ui.click_target(), Some(expected));
            assert_eq!(ui.mouse_down(), vec![InputEvent::Click { h: expected }]);
        }
    }
}
