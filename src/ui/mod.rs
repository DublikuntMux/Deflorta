//! Retained UI: receives element trees from JS, diffs them against the
//! previous tree (enter/exit animations, typewriter state), lays them out with
//! taffy, and produces draw lists and hit tests.

pub mod desc;
pub mod text;

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use taffy as tf;

use crate::assets::Assets;
use desc::{
    Align, AnimDesc, Color, Dim, Edges, Fit, FlexDirection, FlexWrap, NodeDesc, NodeKind, Position,
    Style,
};
use text::{TextStyle, TextSystem};

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}

#[derive(Clone, Copy)]
struct Timed {
    start: Instant,
    spec: AnimDesc,
}

impl Timed {
    /// Eased progress in [0, 1].
    fn progress(&self, now: Instant) -> f32 {
        if self.spec.dur <= 0.0 {
            return 1.0;
        }
        let t = (now.duration_since(self.start).as_secs_f32() / self.spec.dur).clamp(0.0, 1.0);
        // ease-out cubic
        1.0 - (1.0 - t).powi(3)
    }

    fn finished(&self, now: Instant) -> bool {
        now.duration_since(self.start).as_secs_f32() >= self.spec.dur
    }
}

struct Reveal {
    start: Instant,
    cps: f32,
    content: String,
    total: usize,
    done: bool,
}

impl Reveal {
    fn shown(&self, now: Instant) -> usize {
        if self.done {
            return self.total;
        }
        let n = (now.duration_since(self.start).as_secs_f32() * self.cps) as usize;
        n.min(self.total)
    }
}

#[derive(Clone)]
struct Node {
    id: String,
    desc: NodeDesc,
    parent: Option<usize>,
    children: Vec<usize>,
    text_style: TextStyle,
    /// Layout rectangle in virtual units, absolute.
    rect: Rect,
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
}

#[derive(Debug, Clone)]
pub struct Quad {
    pub rect: Rect,
    pub color: Color,
    pub radius: f32,
    pub border_width: f32,
    pub border_color: Color,
    pub image: Option<ImageRef>,
}

#[derive(Debug, Clone)]
pub struct TextDraw {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub color: Color,
}

#[derive(Debug, Clone)]
pub enum DrawItem {
    Quad(Quad),
    Text(TextDraw),
}

/// Maps virtual coordinates to physical pixels: p' = p * s + t.
#[derive(Clone, Copy)]
struct Transform {
    s: f32,
    tx: f32,
    ty: f32,
}

impl Transform {
    fn apply(&self, r: Rect) -> Rect {
        Rect {
            x: r.x * self.s + self.tx,
            y: r.y * self.s + self.ty,
            w: r.w * self.s,
            h: r.h * self.s,
        }
    }

    /// Applies `local` (in virtual units) before `self`.
    fn then_local(&self, k: f32, tx: f32, ty: f32) -> Transform {
        Transform {
            s: k * self.s,
            tx: tx * self.s + self.tx,
            ty: ty * self.s + self.ty,
        }
    }
}

pub struct Ui {
    nodes: Vec<Node>,
    index: HashMap<String, usize>,
    enters: HashMap<String, Timed>,
    reveals: HashMap<String, Reveal>,
    pub text: TextSystem,
    default_font: String,
    virtual_size: (f32, f32),
    scale: f32,
    offset: (f32, f32),
    layout_dirty: bool,
    hovered: HashSet<usize>,
    cursor: Option<(f32, f32)>,
    /// Nodes in draw order with their on-screen rectangles, from the last frame.
    hit_order: Vec<(usize, Rect)>,
    was_revealing: bool,
}

impl Ui {
    pub fn new(text: TextSystem) -> Self {
        Ui {
            nodes: Vec::new(),
            index: HashMap::new(),
            enters: HashMap::new(),
            reveals: HashMap::new(),
            text,
            default_font: String::new(),
            virtual_size: (1280.0, 720.0),
            scale: 1.0,
            offset: (0.0, 0.0),
            layout_dirty: true,
            hovered: HashSet::new(),
            cursor: None,
            hit_order: Vec::new(),
            was_revealing: false,
        }
    }

    pub fn set_config(&mut self, width: f32, height: f32, font: &str) {
        if self.virtual_size != (width, height) || self.default_font != font {
            self.virtual_size = (width.max(1.0), height.max(1.0));
            self.default_font = font.to_owned();
            self.layout_dirty = true;
        }
    }

    /// Updates the letterboxed mapping from virtual units to the physical surface.
    pub fn set_surface_size(&mut self, width: f32, height: f32) {
        let (vw, vh) = self.virtual_size;
        let scale = (width / vw).min(height / vh).max(0.01);
        let offset = (
            ((width - vw * scale) / 2.0).floor(),
            ((height - vh * scale) / 2.0).floor(),
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

    // -----------------------------------------------------------------------
    // Commit
    // -----------------------------------------------------------------------

    /// Replaces the UI tree, starting enter/exit animations for changed keyed elements.
    pub fn commit(
        &mut self,
        tree: NodeDesc,
        instant: bool,
        exits: &HashMap<String, Option<AnimDesc>>,
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

        // Keep removed elements alive while their exit animation plays, at their old z position.
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
                let ghost = match old.ghost {
                    Some(g) if !instant => Some(g),
                    Some(_) => None,
                    None if instant || old.in_ghost => None,
                    None => {
                        let spec = match old.desc.key.as_deref().and_then(|k| exits.get(k)) {
                            Some(spec) => *spec,
                            None => old.desc.exit,
                        };
                        spec.map(|spec| Timed { start: now, spec })
                    }
                };
                if let Some(ghost) = ghost.filter(|g| !g.finished(now)) {
                    let gi = self.copy_ghost(&old_nodes, oc, i, Some(ghost));
                    children.insert(pos, gi);
                    pos += 1;
                }
            }
            self.nodes[i].children = children;
        }

        if instant {
            self.enters.clear();
        } else {
            for node in &self.nodes[..built] {
                if let (Some(spec), false) = (node.desc.enter, old_index.contains_key(&node.id)) {
                    self.enters
                        .insert(node.id.clone(), Timed { start: now, spec });
                }
            }
            self.enters.retain(|id, _| self.index.contains_key(id));
        }

        let mut reveals = HashMap::new();
        for node in &self.nodes[..built] {
            let (Some(content), Some(cps)) = (&node.desc.text, node.desc.cps) else {
                continue;
            };
            if cps <= 0.0 {
                continue;
            }
            let reveal = match self.reveals.remove(&node.id) {
                Some(r) if r.content == *content => r,
                _ => Reveal {
                    start: now,
                    cps,
                    content: content.clone(),
                    total: content.chars().count(),
                    done: instant,
                },
            };
            reveals.insert(node.id.clone(), reveal);
        }
        self.reveals = reveals;

        let live: HashSet<&str> = self.nodes.iter().map(|n| n.id.as_str()).collect();
        self.text.retain(|id| live.contains(id));
        self.hovered.clear();
        self.layout_dirty = true;
    }

    fn build(
        &mut self,
        mut desc: NodeDesc,
        parent: Option<usize>,
        parent_id: &str,
        child_index: usize,
        inherited: &TextStyle,
    ) -> usize {
        let id = match &desc.key {
            Some(key) => format!("{parent_id}/{key}"),
            None => format!("{parent_id}/#{child_index}"),
        };
        let text_style = inherit_text(inherited, &desc.style);
        let children = std::mem::take(&mut desc.children);
        let idx = self.nodes.len();
        self.nodes.push(Node {
            id: id.clone(),
            desc,
            parent,
            children: Vec::new(),
            text_style: text_style.clone(),
            rect: Rect::default(),
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
    // Layout
    // -----------------------------------------------------------------------

    pub fn layout_if_needed(&mut self, assets: &mut Assets) {
        if !self.layout_dirty || self.nodes.is_empty() {
            return;
        }
        self.layout_dirty = false;
        let (vw, vh) = self.virtual_size;
        let mut tree: tf::TaffyTree<usize> = tf::TaffyTree::new();
        let Some(root) = self.build_taffy(&mut tree, 0, true) else {
            return;
        };

        let scale = self.scale;
        let nodes = &self.nodes;
        let reveals = &self.reveals;
        let text = &mut self.text;
        let now = Instant::now();
        let result = tree.compute_layout_with_measure(
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
                            &nodes[i], known, available, scale, text, assets, reveals, now,
                        )
                    },
                )
            },
        );
        if let Err(err) = result {
            eprintln!("[deflorta] layout failed: {err}");
            return;
        }
        self.assign_rects(&tree, root, 0, 0.0, 0.0);
    }

    fn build_taffy(
        &self,
        tree: &mut tf::TaffyTree<usize>,
        i: usize,
        is_root: bool,
    ) -> Option<tf::NodeId> {
        let node = &self.nodes[i];
        let mut style = taffy_style(&node.desc.style);
        if is_root {
            style.position = tf::Position::Relative;
            style.inset = tf::Rect::auto();
            style.size = tf::Size {
                width: tf::Dimension::length(self.virtual_size.0),
                height: tf::Dimension::length(self.virtual_size.1),
            };
        }
        let children: Vec<tf::NodeId> = node
            .children
            .iter()
            .filter(|&&c| !self.nodes[c].in_ghost)
            .filter_map(|&c| self.build_taffy(tree, c, false))
            .collect();
        let id = if children.is_empty() {
            tree.new_leaf_with_context(style, i).ok()?
        } else {
            let id = tree.new_with_children(style, &children).ok()?;
            tree.set_node_context(id, Some(i)).ok()?;
            id
        };
        Some(id)
    }

    fn assign_rects(
        &mut self,
        tree: &tf::TaffyTree<usize>,
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
        if let Some([ax, ay]) = self.nodes[i].desc.anchor {
            rect.x -= ax * rect.w;
            rect.y -= ay * rect.h;
        }
        self.nodes[i].rect = rect;
        let live_children: Vec<usize> = self.nodes[i]
            .children
            .iter()
            .copied()
            .filter(|&c| !self.nodes[c].in_ghost)
            .collect();
        let tchildren = tree.children(tid).unwrap_or_default();
        for (c, tc) in live_children.into_iter().zip(tchildren) {
            self.assign_rects(tree, tc, c, rect.x, rect.y);
        }
    }

    // -----------------------------------------------------------------------
    // Per-frame update and drawing
    // -----------------------------------------------------------------------

    /// True while any animation or typewriter effect is running.
    pub fn is_animating(&self, now: Instant) -> bool {
        self.enters.values().any(|a| !a.finished(now))
            || self
                .nodes
                .iter()
                .any(|n| n.ghost.is_some_and(|g| !g.finished(now)))
            || self.is_revealing(now)
    }

    pub fn is_revealing(&self, now: Instant) -> bool {
        self.reveals.values().any(|r| r.shown(now) < r.total)
    }

    pub fn reveal_all(&mut self) {
        for reveal in self.reveals.values_mut() {
            reveal.done = true;
        }
    }

    /// Returns true once when all typewriter text has finished revealing.
    pub fn take_revealed_event(&mut self, now: Instant) -> bool {
        let revealing = self.is_revealing(now);
        let fired = self.was_revealing && !revealing;
        self.was_revealing = revealing;
        fired
    }

    /// Builds the draw list for this frame (physical pixels).
    pub fn draw(&mut self, assets: &mut Assets, now: Instant) -> Vec<DrawItem> {
        self.layout_if_needed(assets);
        self.hit_order.clear();
        let mut items = Vec::new();
        if self.nodes.is_empty() {
            return items;
        }
        let base = Transform {
            s: self.scale,
            tx: self.offset.0,
            ty: self.offset.1,
        };
        self.draw_node(0, base, 1.0, Color::WHITE, assets, now, &mut items);
        items
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_node(
        &mut self,
        i: usize,
        parent: Transform,
        parent_opacity: f32,
        parent_color: Color,
        assets: &mut Assets,
        now: Instant,
        items: &mut Vec<DrawItem>,
    ) {
        let node = &self.nodes[i];
        if node.desc.style.display == Some(desc::Display::None) {
            return;
        }
        let hover = if self.hovered.contains(&i) {
            node.desc.hover.as_ref()
        } else {
            None
        };
        let pick = |f: fn(&Style) -> Option<f32>| hover.and_then(f).or_else(|| f(&node.desc.style));
        let pick_color =
            |f: fn(&Style) -> Option<Color>| hover.and_then(f).or_else(|| f(&node.desc.style));

        // Animation values: opacity, offset and scale relative to the resting state.
        let (mut opacity, mut dx, mut dy, mut k) = (1.0, 0.0, 0.0, 1.0);
        if let Some(ghost) = node.ghost {
            if ghost.finished(now) {
                return;
            }
            let e = ghost.progress(now);
            let s = ghost.spec;
            opacity = lerp(1.0, s.opacity.unwrap_or(1.0), e);
            dx = lerp(0.0, s.x.unwrap_or(0.0), e);
            dy = lerp(0.0, s.y.unwrap_or(0.0), e);
            k = lerp(1.0, s.scale.unwrap_or(1.0), e);
        } else if let Some(enter) = self.enters.get(&node.id) {
            let e = enter.progress(now);
            let s = enter.spec;
            opacity = lerp(s.opacity.unwrap_or(1.0), 1.0, e);
            dx = lerp(s.x.unwrap_or(0.0), 0.0, e);
            dy = lerp(s.y.unwrap_or(0.0), 0.0, e);
            k = lerp(s.scale.unwrap_or(1.0), 1.0, e);
        }
        let opacity = parent_opacity * opacity * pick(|s| s.opacity).unwrap_or(1.0);
        if opacity <= 0.001 {
            return;
        }
        let k = k * pick(|s| s.scale).unwrap_or(1.0);
        let rect = node.rect;
        let [ax, ay] = node.desc.anchor.unwrap_or([0.5, 0.5]);
        let (pivot_x, pivot_y) = (rect.x + ax * rect.w, rect.y + ay * rect.h);
        let xf = parent.then_local(k, pivot_x * (1.0 - k) + dx, pivot_y * (1.0 - k) + dy);
        let screen = xf.apply(rect);
        if !node.in_ghost {
            self.hit_order.push((i, screen));
        }

        let color = pick_color(|s| s.color).unwrap_or(parent_color);
        let radius = node.desc.style.radius.unwrap_or(0.0) * xf.s;
        let border_width = node.desc.style.border_width.unwrap_or(0.0) * xf.s;
        let border_color = pick_color(|s| s.border_color).unwrap_or(Color([0.0; 4]));
        let background = pick_color(|s| s.background);
        if background.is_some() || border_width > 0.0 {
            items.push(DrawItem::Quad(Quad {
                rect: screen,
                color: background
                    .unwrap_or(Color([0.0; 4]))
                    .with_alpha_mul(opacity),
                radius,
                border_width,
                border_color: border_color.with_alpha_mul(opacity),
                image: None,
            }));
        }

        match node.desc.t {
            NodeKind::Image => {
                if let Some(src) = &node.desc.src
                    && let Some((iw, ih)) = assets.image_size(src)
                {
                    let (dest, uv) = fit_image(screen, iw as f32, ih as f32, node.desc.fit);
                    items.push(DrawItem::Quad(Quad {
                        rect: dest,
                        color: Color::WHITE.with_alpha_mul(opacity),
                        radius,
                        border_width: 0.0,
                        border_color: Color([0.0; 4]),
                        image: Some(ImageRef {
                            src: src.clone(),
                            uv,
                        }),
                    }));
                }
            }
            NodeKind::Text => {
                if let Some(content) = node.desc.text.clone() {
                    let id = node.id.clone();
                    let style = node.text_style.clone();
                    let shadow = node.desc.style.text_shadow;
                    let width = rect.w;
                    let revealed = self.reveals.get(&id).map_or(usize::MAX, |r| r.shown(now));
                    self.text
                        .prepare(&id, &content, &style, self.scale, Some(width), revealed);
                    let text_scale = xf.s / self.scale;
                    if let Some(shadow) = shadow {
                        items.push(DrawItem::Text(TextDraw {
                            id: id.clone(),
                            x: screen.x + shadow.x * xf.s,
                            y: screen.y + shadow.y * xf.s,
                            scale: text_scale,
                            color: shadow.color.with_alpha_mul(opacity),
                        }));
                    }
                    items.push(DrawItem::Text(TextDraw {
                        id,
                        x: screen.x,
                        y: screen.y,
                        scale: text_scale,
                        color: color.with_alpha_mul(opacity),
                    }));
                }
            }
            NodeKind::Box => {}
        }

        let children = self.nodes[i].children.clone();
        for c in children {
            self.draw_node(c, xf, opacity, color, assets, now, items);
        }
    }

    // -----------------------------------------------------------------------
    // Input
    // -----------------------------------------------------------------------

    fn topmost_at(&self, x: f32, y: f32) -> Option<usize> {
        self.hit_order
            .iter()
            .rev()
            .find(|(_, r)| r.contains(x, y))
            .map(|(i, _)| *i)
    }

    /// Updates hover state. Returns true if anything visible changed.
    pub fn pointer_moved(&mut self, x: f32, y: f32) -> bool {
        self.cursor = Some((x, y));
        let mut hovered = HashSet::new();
        let mut cur = self.topmost_at(x, y);
        while let Some(i) = cur {
            hovered.insert(i);
            cur = self.nodes[i].parent;
        }
        let changed = hovered
            .symmetric_difference(&self.hovered)
            .any(|&i| self.nodes[i].desc.hover.is_some());
        self.hovered = hovered;
        changed
    }

    /// Returns the click handler for a press at (x, y), bubbling up from the topmost element.
    pub fn click_target(&self, x: f32, y: f32) -> Option<u32> {
        let mut cur = self.topmost_at(x, y);
        while let Some(i) = cur {
            if let Some(h) = self.nodes[i].desc.on_click {
                return Some(h);
            }
            cur = self.nodes[i].parent;
        }
        None
    }

    /// Recomputes hover after the tree or layout changed.
    pub fn refresh_hover(&mut self) -> bool {
        match self.cursor {
            Some((x, y)) => self.pointer_moved(x, y),
            None => false,
        }
    }

    /// Removes finished ghosts and animations so the tree stays small.
    pub fn prune(&mut self, now: Instant) {
        self.enters.retain(|_, a| !a.finished(now));
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn inherit_text(parent: &TextStyle, style: &Style) -> TextStyle {
    TextStyle {
        font_size: style.font_size.unwrap_or(parent.font_size),
        line_height: style.line_height.unwrap_or(parent.line_height),
        family: style
            .font_family
            .clone()
            .unwrap_or_else(|| parent.family.clone()),
        weight: style.font_weight.map_or(parent.weight, |w| w.value()),
        italic: style.italic.unwrap_or(parent.italic),
        align: style.text_align.or(parent.align),
    }
}

/// Returns the destination rectangle and texture coordinates for an image.
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
                (rect, [(1.0 - u) / 2.0, 0.0, (1.0 + u) / 2.0, 1.0])
            } else {
                let v = img_aspect / box_aspect;
                (rect, [0.0, (1.0 - v) / 2.0, 1.0, (1.0 + v) / 2.0])
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

#[allow(clippy::too_many_arguments)]
fn measure(
    node: &Node,
    known: tf::Size<Option<f32>>,
    available: tf::Size<tf::AvailableSpace>,
    scale: f32,
    text: &mut TextSystem,
    assets: &mut Assets,
    reveals: &HashMap<String, Reveal>,
    now: Instant,
) -> tf::Size<f32> {
    if let tf::Size {
        width: Some(width),
        height: Some(height),
    } = known
    {
        return tf::Size { width, height };
    }
    match node.desc.t {
        NodeKind::Text => {
            let content = node.desc.text.as_deref().unwrap_or("");
            let limit = known.width.or(match available.width {
                tf::AvailableSpace::Definite(w) => Some(w),
                tf::AvailableSpace::MinContent => Some(0.0),
                tf::AvailableSpace::MaxContent => None,
            });
            let revealed = reveals.get(&node.id).map_or(usize::MAX, |r| r.shown(now));
            let (w, h) = text.prepare(&node.id, content, &node.text_style, scale, limit, revealed);
            // Round up so the final layout width never wraps differently than the measurement.
            tf::Size {
                width: known.width.unwrap_or((w / scale).ceil() + 1.0),
                height: known.height.unwrap_or(h / scale),
            }
        }
        NodeKind::Image => {
            let Some((iw, ih)) = node.desc.src.as_deref().and_then(|s| assets.image_size(s)) else {
                return tf::Size::ZERO;
            };
            let (iw, ih) = (iw as f32, ih as f32);
            match (known.width, known.height) {
                (Some(w), None) => tf::Size {
                    width: w,
                    height: w * ih / iw,
                },
                (None, Some(h)) => tf::Size {
                    width: h * iw / ih,
                    height: h,
                },
                _ => tf::Size {
                    width: iw,
                    height: ih,
                },
            }
        }
        NodeKind::Box => tf::Size::ZERO,
    }
}

fn dimension(d: Option<Dim>) -> tf::Dimension {
    match d {
        Some(Dim::Px(v)) => tf::Dimension::length(v),
        Some(Dim::Percent(v)) => tf::Dimension::percent(v),
        Some(Dim::Auto) | None => tf::Dimension::auto(),
    }
}

fn inset(d: Option<Dim>) -> tf::LengthPercentageAuto {
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

fn taffy_style(s: &Style) -> tf::Style {
    let border = s.border_width.unwrap_or(0.0);
    tf::Style {
        display: if s.display == Some(desc::Display::None) {
            tf::Display::None
        } else {
            tf::Display::Flex
        },
        position: if s.position == Some(Position::Absolute) {
            tf::Position::Absolute
        } else {
            tf::Position::Relative
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
            width: tf::LengthPercentage::length(s.gap.unwrap_or(0.0)),
            height: tf::LengthPercentage::length(s.gap.unwrap_or(0.0)),
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
        ..Default::default()
    }
}
