//! Pointer, wheel, keyboard-focus and text-input handling.

use std::collections::HashSet;

use super::desc::{NodeKind, Overflow};
use super::{Rect, Ui};

/// Interaction results the engine forwards to script handlers.
#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    Click { h: u32 },
    Change { h: u32, value: f32 },
    Input { h: u32, value: String },
    Submit { h: u32, value: String },
    Tooltip(Option<String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    Up,
    Down,
    Left,
    Right,
}

/// Pixels scrolled per wheel line, in virtual units.
const WHEEL_STEP: f32 = 48.0;

impl Ui {
    fn topmost_at(&self, x: f32, y: f32) -> Option<usize> {
        self.hit_order
            .iter()
            .rev()
            .find(|(_, r)| r.contains(x, y))
            .map(|(i, _)| *i)
    }

    fn screen_rect(&self, i: usize) -> Option<Rect> {
        self.hit_order
            .iter()
            .find(|(n, _)| *n == i)
            .map(|(_, r)| *r)
    }

    /// The node and its ancestors, innermost first.
    fn ancestors(&self, i: usize) -> impl Iterator<Item = usize> + '_ {
        std::iter::successors(Some(i), |&n| self.nodes[n].parent)
    }

    fn id_index(&self, id: Option<&str>) -> Option<usize> {
        id.and_then(|id| self.index.get(id)).copied()
    }

    /// Pointer moved (physical pixels); `None` when it left the window.
    /// Returns whether a redraw is needed and any events for scripts.
    pub fn pointer_moved(&mut self, position: Option<(f32, f32)>) -> (bool, Vec<InputEvent>) {
        if position != self.cursor && position.is_some() && self.focused_input().is_none() {
            // Mouse use takes over from keyboard focus.
            self.focused = None;
        }
        self.cursor = position;
        self.refresh_hover()
    }

    /// Recomputes hover, slider dragging and tooltips for the current pointer
    /// position (also after the tree or layout changed).
    pub fn refresh_hover(&mut self) -> (bool, Vec<InputEvent>) {
        let mut events = Vec::new();
        let (x, y) = self
            .cursor
            .unwrap_or((f32::NEG_INFINITY, f32::NEG_INFINITY));
        let hovered: HashSet<usize> = self
            .topmost_at(x, y)
            .map_or_default(|t| self.ancestors(t).collect());
        let mut redraw = hovered
            .symmetric_difference(&self.hovered)
            .any(|&i| self.nodes[i].desc.hover.is_some() || self.nodes[i].desc.hover_src.is_some());
        self.hovered = hovered;

        if let Some(i) = self.id_index(self.dragging.as_deref())
            && let Some(event) = self.slider_value_at(i, x)
        {
            events.push(event);
            redraw = true;
        }
        if let Some(event) = self.update_tooltip() {
            events.push(event);
        }
        (redraw, events)
    }

    fn update_tooltip(&mut self) -> Option<InputEvent> {
        let source = self.id_index(self.focused.as_deref()).or_else(|| {
            let (x, y) = self.cursor?;
            self.topmost_at(x, y)
        });
        let tooltip = source.and_then(|i| {
            self.ancestors(i)
                .find_map(|n| self.nodes[n].desc.tooltip.clone())
        });
        if tooltip == self.tooltip {
            return None;
        }
        self.tooltip.clone_from(&tooltip);
        Some(InputEvent::Tooltip(tooltip))
    }

    // Emit every distinct snapped value; an epsilon could hide small valid steps.
    #[allow(clippy::float_cmp)]
    fn slider_value_at(&self, i: usize, x: f32) -> Option<InputEvent> {
        let node = &self.nodes[i];
        let h = node.desc.on_change?;
        let rect = self.screen_rect(i)?;
        let (min, max) = (node.desc.min.unwrap_or(0.0), node.desc.max.unwrap_or(1.0));
        let frac = ((x - rect.x) / rect.w.max(1.0)).clamp(0.0, 1.0);
        let value = snap(frac.mul_add(max - min, min), min, max, node.desc.step);
        (value != node.desc.number_value()).then_some(InputEvent::Change { h, value })
    }

    /// Press at the current pointer position.
    pub fn mouse_down(&mut self) -> Vec<InputEvent> {
        let Some((x, y)) = self.cursor else {
            return Vec::new();
        };
        let Some(target) = self.topmost_at(x, y) else {
            return Vec::new();
        };
        let chain: Vec<usize> = self.ancestors(target).collect();
        let mut events = Vec::new();
        if !chain
            .iter()
            .any(|&n| self.nodes[n].desc.kind() == NodeKind::Input)
        {
            self.focused = None;
        }
        for n in chain {
            let desc = &self.nodes[n].desc;
            match desc.kind() {
                NodeKind::Slider if desc.on_change.is_some() => {
                    self.dragging = Some(self.nodes[n].id.clone());
                    events.extend(self.slider_value_at(n, x));
                    return events;
                }
                NodeKind::Input => {
                    self.focused = Some(self.nodes[n].id.clone());
                    return events;
                }
                _ => {}
            }
            if let Some(h) = desc.on_click {
                events.push(InputEvent::Click { h });
                return events;
            }
        }
        events
    }

    /// The click handler under the pointer, bubbling up from the topmost element.
    pub fn click_target(&self) -> Option<u32> {
        let (x, y) = self.cursor?;
        let target = self.topmost_at(x, y)?;
        self.ancestors(target)
            .find_map(|n| self.nodes[n].desc.on_click)
    }

    pub fn mouse_up(&mut self) {
        self.dragging = None;
    }

    /// Scrolls the innermost scroll container under the pointer that can move
    /// in that direction. Returns false when nothing scrolled.
    pub fn scroll_at(&mut self, dy: f32) -> bool {
        let Some((x, y)) = self.cursor else {
            return false;
        };
        let Some(target) = self.topmost_at(x, y) else {
            return false;
        };
        let chain: Vec<usize> = self.ancestors(target).collect();
        for n in chain {
            let node = &self.nodes[n];
            if node.desc.style.overflow != Some(Overflow::Scroll) {
                continue;
            }
            let max = (node.content.1 - node.rect.h).max(0.0);
            let entry = self.scroll.entry(node.id.clone()).or_insert((0.0, 0.0));
            let next = dy.mul_add(WHEEL_STEP, entry.1).clamp(0.0, max);
            if (next - entry.1).abs() > 0.01 {
                entry.1 = next;
                return true;
            }
        }
        false
    }

    // -----------------------------------------------------------------------
    // Keyboard / gamepad focus
    // -----------------------------------------------------------------------

    /// Focusable nodes that are visible and not covered by something on top.
    fn focus_candidates(&self) -> Vec<(usize, Rect)> {
        self.hit_order
            .iter()
            .filter(|(i, r)| {
                let node = &self.nodes[*i];
                if !node.desc.is_focusable() || node.in_ghost || r.w < 1.0 || r.h < 1.0 {
                    return false;
                }
                let (cx, cy) = r.center();
                self.topmost_at(cx, cy)
                    .is_some_and(|top| self.ancestors(top).any(|a| a == *i))
            })
            .copied()
            .collect()
    }

    /// Moves focus in a direction, or adjusts a focused slider. Returns events
    /// and whether the key was consumed.
    pub fn navigate(&mut self, nav: Nav) -> (bool, Vec<InputEvent>) {
        if let Some(i) = self.id_index(self.focused.as_deref())
            && self.nodes[i].desc.kind() == NodeKind::Slider
            && matches!(nav, Nav::Left | Nav::Right)
            && let Some(h) = self.nodes[i].desc.on_change
        {
            let desc = &self.nodes[i].desc;
            let (min, max) = (desc.min.unwrap_or(0.0), desc.max.unwrap_or(1.0));
            let step = desc.step.unwrap_or((max - min) / 20.0);
            let delta = if nav == Nav::Left { -step } else { step };
            let value = snap(desc.number_value() + delta, min, max, desc.step);
            return (true, vec![InputEvent::Change { h, value }]);
        }

        let candidates = self.focus_candidates();
        if candidates.is_empty() {
            return (false, Vec::new());
        }
        let current = self
            .id_index(self.focused.as_deref())
            .and_then(|i| candidates.iter().find(|(n, _)| *n == i));
        let next = match current {
            None => candidates
                .iter()
                .find(|(i, _)| self.nodes[*i].desc.autofocus)
                .or_else(|| {
                    candidates
                        .iter()
                        .min_by(|a, b| (a.1.y, a.1.x).partial_cmp(&(b.1.y, b.1.x)).unwrap())
                })
                .map(|(i, _)| *i),
            Some(&(ci, cr)) => {
                let (fx, fy) = cr.center();
                candidates
                    .iter()
                    .filter(|(i, _)| *i != ci)
                    .filter_map(|(i, r)| {
                        let (cx, cy) = r.center();
                        let (dx, dy) = (cx - fx, cy - fy);
                        let (primary, ortho) = match nav {
                            Nav::Right => (dx, dy.abs()),
                            Nav::Left => (-dx, dy.abs()),
                            Nav::Down => (dy, dx.abs()),
                            Nav::Up => (-dy, dx.abs()),
                        };
                        (primary > 1.0).then_some((*i, f32::mul_add(ortho, 2.0, primary)))
                    })
                    .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                    .map(|(i, _)| i)
            }
        };
        let Some(next) = next else {
            return (current.is_some(), Vec::new());
        };
        self.focused = Some(self.nodes[next].id.clone());
        self.scroll_into_view(next);
        (true, self.update_tooltip().into_iter().collect())
    }

    fn scroll_into_view(&mut self, i: usize) {
        let rect = self.nodes[i].rect;
        let containers: Vec<usize> = self
            .ancestors(i)
            .skip(1)
            .filter(|&n| self.nodes[n].desc.style.overflow == Some(Overflow::Scroll))
            .collect();
        for c in containers {
            let container = self.nodes[c].rect;
            let max = (self.nodes[c].content.1 - container.h).max(0.0);
            let entry = self
                .scroll
                .entry(self.nodes[c].id.clone())
                .or_insert((0.0, 0.0));
            let top = rect.y - container.y;
            if top < entry.1 {
                entry.1 = top.max(0.0);
            } else if top + rect.h > entry.1 + container.h {
                entry.1 = (top + rect.h - container.h).min(max);
            }
        }
    }

    pub fn has_focus(&self) -> bool {
        self.id_index(self.focused.as_deref()).is_some()
    }

    pub fn clear_focus(&mut self) {
        self.focused = None;
    }

    /// Activates the focused element (Enter / gamepad A).
    pub fn activate(&self) -> Option<InputEvent> {
        let i = self.id_index(self.focused.as_deref())?;
        let desc = &self.nodes[i].desc;
        if desc.kind() == NodeKind::Input {
            let h = desc.on_submit?;
            return Some(InputEvent::Submit {
                h,
                value: desc.string_value().to_owned(),
            });
        }
        desc.on_click.map(|h| InputEvent::Click { h })
    }

    // -----------------------------------------------------------------------
    // Text input
    // -----------------------------------------------------------------------

    pub fn focused_input(&self) -> Option<usize> {
        self.id_index(self.focused.as_deref())
            .filter(|&i| self.nodes[i].desc.kind() == NodeKind::Input)
    }

    /// Appends typed text to the focused input.
    pub fn type_text(&mut self, text: &str) -> Option<InputEvent> {
        let i = self.focused_input()?;
        let desc = &mut self.nodes[i].desc;
        let mut value = desc.string_value().to_owned();
        for c in text.chars().filter(|c| !c.is_control()) {
            if desc
                .max_length
                .is_some_and(|max| value.chars().count() >= max)
            {
                break;
            }
            value.push(c);
        }
        self.set_input_value(i, value)
    }

    pub fn backspace(&mut self) -> Option<InputEvent> {
        let i = self.focused_input()?;
        let mut value = self.nodes[i].desc.string_value().to_owned();
        value.pop()?;
        self.set_input_value(i, value)
    }

    fn set_input_value(&mut self, i: usize, value: String) -> Option<InputEvent> {
        let desc = &mut self.nodes[i].desc;
        if desc.string_value() == value {
            return None;
        }
        // Show the edit immediately; the script's re-render confirms it.
        desc.value = Some(serde_json::Value::String(value.clone()));
        self.layout_dirty = true;
        desc.on_input.map(|h| InputEvent::Input { h, value })
    }
}

fn snap(value: f32, min: f32, max: f32, step: Option<f32>) -> f32 {
    let value = match step {
        Some(step) if step > 0.0 => ((value - min) / step).round().mul_add(step, min),
        _ => value,
    };
    value.clamp(min.min(max), max.max(min))
}
