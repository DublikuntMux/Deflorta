use std::collections::{HashMap, HashSet};

use accesskit::{
    Action, ActionData, ActionRequest, Live, Node, NodeId, Role, TreeInfo, TreeUpdate,
};
use num_traits::{AsPrimitive, ToPrimitive};

use super::desc::NodeKind;
use super::{InputEvent, Ui};
use crate::self_voicing::SpeechSnapshot;
use crate::util::math::snap;

const ROOT: NodeId = NodeId(0);

#[derive(Default)]
pub(super) struct Accessibility {
    ids: HashMap<String, NodeId>,
    text_runs: HashMap<String, NodeId>,
    next_id: u64,
    /// Only targets in the last published tree can receive actions.
    exposed: HashSet<NodeId>,
}

impl Ui {
    fn modal_start(&self) -> usize {
        self.hit_order
            .iter()
            .rposition(|(i, _)| self.nodes[*i].desc.accessibility.modal)
            .unwrap_or(0)
    }

    fn accessibility_indices(&self) -> HashSet<usize> {
        self.hit_order[self.modal_start()..]
            .iter()
            .map(|(i, _)| *i)
            .collect()
    }

    pub(super) fn accessibility_exposed(&self, i: usize) -> bool {
        !self.nodes[i].in_ghost
            && self.hit_order[self.modal_start()..]
                .iter()
                .any(|(n, _)| *n == i)
    }

    fn accessible_text(&self, i: usize, exposed: &HashSet<usize>) -> String {
        let node = &self.nodes[i];
        if let Some(label) = &node.desc.accessibility.label {
            return label.clone();
        }
        if let Some(alt) = &node.desc.accessibility.alt {
            return alt.clone();
        }
        if let Some(spans) = &node.spans {
            return spans.iter().map(|span| span.text.as_str()).collect();
        }
        if node.desc.kind() == NodeKind::Input {
            return node.desc.placeholder.clone().unwrap_or_default();
        }
        node.children
            .iter()
            .filter(|child| exposed.contains(child) && !self.accessible_control(**child))
            .map(|&child| self.accessible_text(child, exposed))
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn accessible_label(&self, i: usize, exposed: &HashSet<usize>) -> String {
        let text = self.accessible_text(i, exposed);
        if text.is_empty() && self.nodes[i].desc.accessibility.alt.as_deref() != Some("") {
            return self.nodes[i].desc.tooltip.clone().unwrap_or_default();
        }
        text
    }

    fn accessible_control(&self, i: usize) -> bool {
        let desc = &self.nodes[i].desc;
        desc.is_focusable()
            || desc.focus.disabled
            || desc.on_click.is_some() && i != 0 && !desc.accessibility.modal
    }

    /// Full trees are required by the winit event-loop-proxy adapter.
    pub fn accessibility_update(&mut self, title: &str) -> TreeUpdate {
        self.accessibility
            .ids
            .retain(|key, _| self.index.contains_key(key));
        self.accessibility
            .text_runs
            .retain(|key, _| self.index.contains_key(key));
        let exposed_set = self.accessibility_indices();
        let exposed: Vec<usize> = self
            .hit_order
            .iter()
            .filter(|(i, _)| exposed_set.contains(i))
            .map(|(i, _)| *i)
            .collect();
        for &i in &exposed {
            let key = &self.nodes[i].id;
            if !self.accessibility.ids.contains_key(key) {
                self.accessibility.next_id += 1;
                self.accessibility
                    .ids
                    .insert(key.clone(), NodeId(self.accessibility.next_id));
            }
            if self.nodes[i].desc.kind() == NodeKind::Input
                && !self.accessibility.text_runs.contains_key(key)
            {
                self.accessibility.next_id += 1;
                self.accessibility
                    .text_runs
                    .insert(key.clone(), NodeId(self.accessibility.next_id));
            }
        }
        self.accessibility.exposed = exposed
            .iter()
            .map(|&i| self.accessibility.ids[&self.nodes[i].id])
            .collect();
        let mut root = Node::new(Role::Window);
        root.set_label(title);
        root.set_children(
            exposed
                .iter()
                .filter(|&&i| {
                    self.nodes[i]
                        .parent
                        .is_none_or(|parent| !exposed_set.contains(&parent))
                })
                .map(|&i| self.accessibility.ids[&self.nodes[i].id])
                .collect::<Vec<_>>(),
        );
        let mut nodes = vec![(ROOT, root)];
        for &i in &exposed {
            let mut node = self.accessible_node(i, &exposed_set);
            if self.nodes[i].desc.kind() == NodeKind::Input {
                let id = self.accessibility.text_runs[&self.nodes[i].id];
                let value = self.nodes[i].desc.string_value();
                let mut run = Node::new(Role::TextRun);
                run.set_value(value);
                run.set_character_lengths(
                    value
                        .chars()
                        // Every UTF-8 character occupies at most four bytes.
                        .map(|c| c.len_utf8().as_())
                        .collect::<Vec<_>>(),
                );
                if let Some(bounds) = node.bounds() {
                    run.set_bounds(bounds);
                }
                node.push_child(id);
                let caret = accesskit::TextPosition {
                    node: id,
                    character_index: value.chars().count(),
                };
                node.set_text_selection(accesskit::TextSelection {
                    anchor: caret,
                    focus: caret,
                });
                nodes.push((id, run));
            }
            nodes.push((self.accessibility.ids[&self.nodes[i].id], node));
        }
        let focus = self
            .focused
            .as_ref()
            .and_then(|key| self.accessibility.ids.get(key))
            .filter(|id| self.accessibility.exposed.contains(id))
            .copied()
            .unwrap_or(ROOT);
        TreeUpdate {
            nodes,
            tree: Some(TreeInfo::new(ROOT)),
            tree_id: accesskit::TreeId::ROOT,
            focus,
        }
    }

    fn accessible_role(&self, i: usize) -> Role {
        let desc = &self.nodes[i].desc;
        let named_parent = self
            .ancestors(i)
            .skip(1)
            .any(|a| self.accessible_control(a));
        match desc.kind() {
            NodeKind::Slider => Role::Slider,
            NodeKind::Input => Role::TextInput,
            _ if self.accessible_control(i) => Role::Button,
            NodeKind::Text | NodeKind::Image if named_parent => Role::GenericContainer,
            NodeKind::Text => Role::Label,
            NodeKind::Image
                if desc
                    .accessibility
                    .alt
                    .as_deref()
                    .is_some_and(|alt| !alt.is_empty()) =>
            {
                Role::Image
            }
            _ if desc.accessibility.modal => Role::Dialog,
            _ if desc.accessibility.live => Role::Group,
            _ => Role::GenericContainer,
        }
    }

    fn accessible_node(&self, i: usize, exposed_set: &HashSet<usize>) -> Node {
        let desc = &self.nodes[i].desc;
        let role = self.accessible_role(i);
        let mut node = Node::new(role);
        if role != Role::GenericContainer && role != Role::Group && role != Role::Dialog {
            node.set_label(self.accessible_label(i, exposed_set));
        }
        node.set_children(
            self.nodes[i]
                .children
                .iter()
                .filter(|child| exposed_set.contains(child))
                .map(|&child| self.accessibility.ids[&self.nodes[child].id])
                .collect::<Vec<_>>(),
        );
        if let Some((_, rect)) = self.hit_order.iter().find(|(n, _)| *n == i) {
            node.set_bounds(accesskit::Rect::new(
                f64::from(rect.x),
                f64::from(rect.y),
                f64::from(rect.x + rect.w),
                f64::from(rect.y + rect.h),
            ));
        }
        if desc.focus.disabled {
            node.set_disabled();
        }
        if desc.accessibility.live {
            node.set_live(Live::Polite);
            node.set_live_atomic();
        }
        if desc.accessibility.modal {
            node.set_modal();
        }
        node.add_action(Action::ScrollIntoView);
        if desc.is_focusable() && !desc.focus.disabled {
            node.add_action(Action::Focus);
            node.add_action(Action::Blur);
        }
        if desc.on_click.is_some() && !desc.focus.disabled {
            node.add_action(Action::Click);
        }
        match desc.kind() {
            NodeKind::Slider => {
                let min = desc.min.unwrap_or(0.0);
                let max = desc.max.unwrap_or(1.0);
                node.set_numeric_value(f64::from(desc.number_value()));
                node.set_min_numeric_value(f64::from(min));
                node.set_max_numeric_value(f64::from(max));
                node.set_numeric_value_step(f64::from(desc.step.unwrap_or((max - min) / 20.0)));
                if desc.on_change.is_some() && !desc.focus.disabled {
                    node.add_action(Action::SetValue);
                    node.add_action(Action::Increment);
                    node.add_action(Action::Decrement);
                }
            }
            NodeKind::Input => {
                node.set_value(desc.string_value());
                if let Some(placeholder) = &desc.placeholder {
                    node.set_placeholder(placeholder);
                }
                if !desc.focus.disabled {
                    node.add_action(Action::SetValue);
                }
            }
            _ => {}
        }
        node
    }

    pub fn accessibility_action(&mut self, request: ActionRequest) -> Vec<InputEvent> {
        if request.target_tree != accesskit::TreeId::ROOT
            || !self.accessibility.exposed.contains(&request.target_node)
        {
            return Vec::new();
        }
        let Some(i) = self
            .accessibility
            .ids
            .iter()
            .find(|(_, id)| **id == request.target_node)
            .and_then(|(key, _)| self.index.get(key))
            .copied()
        else {
            return Vec::new();
        };
        if !self.accessibility_exposed(i) || self.nodes[i].desc.focus.disabled {
            return Vec::new();
        }
        let desc = &self.nodes[i].desc;
        match request.action {
            Action::Focus if desc.is_focusable() => {
                self.focused = Some(self.nodes[i].id.clone());
                self.scroll_into_view(i);
                self.update_tooltip().into_iter().collect()
            }
            Action::Blur if self.focused.as_deref() == Some(self.nodes[i].id.as_str()) => {
                self.clear_focus();
                self.update_tooltip().into_iter().collect()
            }
            Action::ScrollIntoView => {
                self.scroll_into_view(i);
                Vec::new()
            }
            Action::Click => desc
                .on_click
                .map(|h| InputEvent::Click { h })
                .into_iter()
                .collect(),
            Action::SetValue | Action::Increment | Action::Decrement
                if desc.kind() == NodeKind::Slider =>
            {
                let Some(h) = desc.on_change else {
                    return Vec::new();
                };
                let min = desc.min.unwrap_or(0.0);
                let max = desc.max.unwrap_or(1.0);
                let step = desc.step.unwrap_or((max - min) / 20.0);
                let value = match request.action {
                    Action::Increment => desc.number_value() + step,
                    Action::Decrement => desc.number_value() - step,
                    _ => match request.data {
                        Some(ActionData::NumericValue(value)) if value.is_finite() => {
                            let Some(value) = value.to_f32().filter(|v| v.is_finite()) else {
                                return Vec::new();
                            };
                            value
                        }
                        _ => return Vec::new(),
                    },
                };
                let value = snap(value, min, max, desc.step);
                vec![InputEvent::Change { h, value }]
            }
            Action::SetValue if desc.kind() == NodeKind::Input => {
                let Some(ActionData::Value(value)) = request.data else {
                    return Vec::new();
                };
                self.replace_input_text(i, &value).into_iter().collect()
            }
            _ => Vec::new(),
        }
    }

    /// Whole text is used, independently of the typewriter's revealed prefix.
    pub fn speech_snapshot(&self) -> SpeechSnapshot {
        let mut snapshot = SpeechSnapshot::default();
        let exposed = self.accessibility_indices();
        for &(i, rect) in &self.hit_order {
            if !exposed.contains(&i) || rect.w <= 0.0 || rect.h <= 0.0 {
                continue;
            }
            let node = &self.nodes[i];
            let control = self.accessible_control(i);
            let text = node.desc.kind() == NodeKind::Text;
            let image =
                node.desc.kind() == NodeKind::Image && node.desc.accessibility.alt.is_some();
            // Button labels already include their noninteractive descendants.
            if !(control || text || image)
                || self
                    .ancestors(i)
                    .skip(1)
                    .any(|a| self.accessible_control(a))
                    && !control
            {
                continue;
            }
            let mut label = self.accessible_label(i, &exposed);
            match node.desc.kind() {
                NodeKind::Slider => label = format!("{label}, {}", node.desc.number_value()),
                NodeKind::Input => label = format!("{label}, {}", node.desc.string_value()),
                _ => {}
            }
            if label.trim().is_empty() {
                continue;
            }
            snapshot.content.push((node.id.clone(), label.clone()));
            if control
                && (self.focused.as_deref() == Some(node.id.as_str())
                    || self.focused.is_none() && self.hovered.contains(&i))
            {
                snapshot.target = Some((node.id.clone(), label));
            }
        }
        snapshot
    }
}

#[cfg(test)]
mod tests;
