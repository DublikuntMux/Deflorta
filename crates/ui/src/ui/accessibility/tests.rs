use super::*;
use crate::assets::Assets;
use crate::ui::desc::NodeDesc;
use crate::ui::text::TextSystem;
use deflorta_common::handler::Handler;
use std::time::Instant;

fn setup() -> (Ui, Assets) {
    let files = crate::GameFiles::open(&crate::workspace_dir().join("game")).unwrap();
    (Ui::new(TextSystem::new(&files)), Assets::new(files))
}

fn draw(ui: &mut Ui, assets: &mut Assets, tree: NodeDesc) {
    let now = Instant::now();
    ui.commit(tree, true, &HashMap::new(), assets, now);
    ui.draw(assets, now);
}

fn fixture() -> NodeDesc {
    let mut tree: NodeDesc = serde_json::from_value(serde_json::json!({
        "key": "root", "style": {"flexDirection": "column"}, "children": [
            {"key": "image", "t": "image", "alt": "Save game", "src": "button.png", "style": {"width": 100, "height": 50}},
            {"key": "slider", "t": "slider", "label": "Music volume", "value": 0.5, "min": 0, "max": 1, "step": 0.1, "style": {"width": 100, "height": 30}},
            {"key": "input", "t": "input", "label": "Your name", "value": "", "maxLength": 3},
            {"key": "line", "t": "text", "live": true, "cps": 1, "spans": [{"text": "Hello "}, {"text": "world", "b": true}]},
            {"key": "hidden", "t": "text", "text": "Hidden", "style": {"display": "none"}},
            {"key": "decorative", "t": "image", "alt": "", "style": {"width": 10, "height": 10}}
        ]
    })).unwrap();
    let handler = Handler {
        generation: 1,
        index: 2,
    };
    tree.children[0].on_click = Some(handler);
    tree.children[1].on_change = Some(handler);
    tree.children[2].on_input = Some(handler);
    tree
}

fn action(target: NodeId, action: Action, data: Option<ActionData>) -> ActionRequest {
    ActionRequest {
        target_node: target,
        target_tree: accesskit::TreeId::ROOT,
        action,
        data,
    }
}

#[test]
fn semantics_names_values_and_stable_ids_follow_the_ui() {
    let (mut ui, mut assets) = setup();
    draw(&mut ui, &mut assets, fixture());
    let tree = ui.accessibility_update("Game");
    let named = |label| {
        tree.nodes
            .iter()
            .find(|(_, n)| n.label() == Some(label))
            .unwrap()
    };
    let (image, button) = named("Save game");
    assert_eq!(button.role(), Role::Button);
    assert!(button.supports_action(Action::Click));
    let (_, slider) = named("Music volume");
    assert_eq!(slider.role(), Role::Slider);
    assert_eq!(slider.numeric_value(), Some(0.5));
    assert_eq!(slider.min_numeric_value(), Some(0.0));
    assert_eq!(slider.max_numeric_value(), Some(1.0));
    assert_eq!(named("Your name").1.role(), Role::TextInput);
    let consumer = accesskit_consumer::Tree::new(tree.clone(), true);
    let root = consumer.state().root().children().next().unwrap();
    let input = root
        .children()
        .find(|n| n.role() == Role::TextInput)
        .unwrap();
    assert!(input.supports_text_ranges());
    assert_eq!(input.document_range().text(), "");
    assert_eq!(named("Hello world").1.live(), Some(Live::Polite));
    assert!(tree.nodes.iter().all(|(_, n)| n.label() != Some("Hidden")));
    assert!(tree.nodes.iter().all(|(_, n)| n.role() != Role::Image));
    assert!(
        ui.speech_snapshot()
            .content
            .iter()
            .any(|(_, text)| text == "Hello world")
    );
    let mut next = fixture();
    next.children.swap(0, 1);
    draw(&mut ui, &mut assets, next);
    let updated = ui.accessibility_update("Game");
    assert_eq!(
        updated
            .nodes
            .iter()
            .find(|(_, n)| n.label() == Some("Save game"))
            .unwrap()
            .0,
        *image
    );
}

#[test]
fn actions_use_current_handlers_and_validate_values_and_targets() {
    let (mut ui, mut assets) = setup();
    draw(&mut ui, &mut assets, fixture());
    let tree = ui.accessibility_update("Game");
    let id = |label| {
        tree.nodes
            .iter()
            .find(|(_, n)| n.label() == Some(label))
            .unwrap()
            .0
    };
    let image = id("Save game");
    assert!(matches!(
        ui.accessibility_action(action(image, Action::Click, None))[0],
        InputEvent::Click { .. }
    ));
    ui.accessibility_action(action(image, Action::Focus, None));
    assert_eq!(ui.accessibility_update("Game").focus, image);
    let slider = id("Music volume");
    assert!(matches!(
        ui.accessibility_action(action(
            slider,
            Action::SetValue,
            Some(ActionData::NumericValue(5.0))
        ))[0],
        InputEvent::Change { value: 1.0, .. }
    ));
    assert_eq!(
        ui.accessibility_action(action(
            slider,
            Action::SetValue,
            Some(ActionData::NumericValue(f64::NAN))
        )),
        []
    );
    let input = id("Your name");
    assert!(
        matches!(&ui.accessibility_action(action(input, Action::SetValue, Some(ActionData::Value("A\n😀BC".into()))))[0], InputEvent::Input { value, .. } if value == "A😀B")
    );
    let consumer = accesskit_consumer::Tree::new(ui.accessibility_update("Game"), true);
    let root = consumer.state().root().children().next().unwrap();
    let input = root
        .children()
        .find(|n| n.role() == Role::TextInput)
        .unwrap();
    assert_eq!(input.document_range().text(), "A😀B");
    assert_eq!(
        ui.accessibility_action(action(NodeId(999), Action::Click, None)),
        []
    );
    draw(&mut ui, &mut assets, NodeDesc::default());
    assert_eq!(
        ui.accessibility_action(action(image, Action::Click, None)),
        []
    );
}

#[test]
fn modal_screens_exclude_underlying_content_and_actions() {
    let (mut ui, mut assets) = setup();
    let mut fixture = fixture();
    draw(&mut ui, &mut assets, fixture.clone());
    let tree = ui.accessibility_update("Game");
    let old = tree
        .nodes
        .iter()
        .find(|(_, n)| n.label() == Some("Save game"))
        .unwrap()
        .0;
    fixture.children.push(
        serde_json::from_value(serde_json::json!({
            "key": "dialog", "modal": true, "style": {"width": 200, "height": 100},
            "children": [{"t": "text", "text": "Confirm save"}]
        }))
        .unwrap(),
    );
    fixture.children.push(
        serde_json::from_value(serde_json::json!({
            "t": "text", "text": "Saved", "live": true
        }))
        .unwrap(),
    );
    draw(&mut ui, &mut assets, fixture);
    let tree = ui.accessibility_update("Game");
    assert!(
        tree.nodes
            .iter()
            .any(|(_, n)| n.role() == Role::Dialog && n.is_modal())
    );
    assert!(
        tree.nodes
            .iter()
            .all(|(_, n)| n.label() != Some("Save game"))
    );
    assert_eq!(
        ui.accessibility_action(action(old, Action::Click, None)),
        []
    );
    assert_eq!(ui.speech_snapshot().content.len(), 2);
    assert!(tree.nodes.iter().any(|(_, n)| n.label() == Some("Saved")));
    assert_eq!(tree.nodes[0].1.children().len(), 2);
}

#[test]
fn entering_input_keeps_autofocus_before_it_is_drawn() {
    let (mut ui, mut assets) = setup();
    let now = Instant::now();
    let tree: NodeDesc = serde_json::from_value(serde_json::json!({
        "key": "dialog", "enter": {"dur": 0.2, "opacity": 0},
        "children": [{"t": "input", "key": "input", "autofocus": true, "label": "Name"}]
    }))
    .unwrap();
    ui.commit(tree, false, &HashMap::new(), &assets, now);
    ui.draw(&mut assets, now);
    assert!(ui.focused_input().is_some());
    ui.draw(&mut assets, now + std::time::Duration::from_secs(1));
    let tree = ui.accessibility_update("Game");
    let input = tree
        .nodes
        .iter()
        .find(|(_, n)| n.label() == Some("Name"))
        .unwrap()
        .0;
    assert_eq!(tree.focus, input);
}

#[test]
fn offscreen_controls_can_be_focused_and_scrolled_into_view() {
    let (mut ui, mut assets) = setup();
    let tree: NodeDesc = serde_json::from_value(serde_json::json!({
        "key": "scroll", "style": {"overflow": "scroll", "width": 200, "height": 50, "flexDirection": "column"},
        "children": [
            {"style": {"height": 200, "flexShrink": 0}},
            {"t": "input", "key": "input", "label": "Offscreen", "style": {"height": 30, "flexShrink": 0}}
        ]
    })).unwrap();
    draw(
        &mut ui,
        &mut assets,
        NodeDesc {
            children: vec![tree],
            ..Default::default()
        },
    );
    let tree = ui.accessibility_update("Game");
    let (id, input) = tree
        .nodes
        .iter()
        .find(|(_, n)| n.label() == Some("Offscreen"))
        .unwrap();
    assert_eq!(input.bounds().unwrap().height(), 0.0);
    let id = *id;
    ui.accessibility_action(action(id, Action::Focus, None));
    ui.draw(&mut assets, Instant::now());
    let tree = ui.accessibility_update("Game");
    assert_eq!(tree.focus, id);
    assert!(
        tree.nodes
            .iter()
            .find(|(node, _)| *node == id)
            .unwrap()
            .1
            .bounds()
            .unwrap()
            .height()
            > 0.0
    );
}
