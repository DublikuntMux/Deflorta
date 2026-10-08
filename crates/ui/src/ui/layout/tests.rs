use super::*;
use deflorta_common::handler::Handler;

fn setup() -> (Ui, Assets) {
    let game = crate::GameFiles::open(&crate::workspace_dir().join("game")).unwrap();
    (Ui::new(TextSystem::new(&game)), Assets::new(game))
}

fn fixture() -> super::super::desc::NodeDesc {
    serde_json::from_value(serde_json::json!({
        "key": "root", "style": { "gap": 20 }, "children": [
            { "key": "panel", "style": { "width": 300, "flexDirection": "column" }, "children": [
                { "key": "label", "t": "text", "text": "Short text" },
                { "key": "field", "t": "input", "value": "hi", "autofocus": true }
            ] },
            { "key": "sibling", "style": { "width": 300, "height": 100 } }
        ]
    }))
    .unwrap()
}

fn commit(ui: &mut Ui, tree: super::super::desc::NodeDesc, assets: &Assets, now: Instant) {
    ui.commit(tree, true, &HashMap::new(), assets, now);
}

fn assert_matches_fresh(
    ui: &mut Ui,
    tree: super::super::desc::NodeDesc,
    assets: &mut Assets,
    now: Instant,
) {
    ui.draw(assets, now);
    let (mut fresh, mut fresh_assets) = setup();
    fresh.set_config(ui.virtual_size.0, ui.virtual_size.1, &ui.default_font);
    fresh.scale = ui.scale;
    commit(&mut fresh, tree, &fresh_assets, now);
    fresh.draw(&mut fresh_assets, now);
    for node in ui.nodes.iter().filter(|n| !n.in_ghost) {
        let expected = &fresh.nodes[fresh.index[&node.id]];
        assert_eq!(node.rect, expected.rect, "rectangle for {}", node.id);
        assert_eq!(node.content, expected.content, "content for {}", node.id);
    }
}

#[test]
fn paint_and_handler_updates_skip_layout() {
    let (mut ui, mut assets) = setup();
    let now = Instant::now();
    let mut tree = fixture();
    commit(&mut ui, tree.clone(), &assets, now);
    ui.draw(&mut assets, now);
    let ids: Vec<_> = ui.nodes.iter().map(|n| n.layout_id).collect();
    tree.style.background = Some(super::super::desc::Color::WHITE);
    tree.style.color = Some(super::super::desc::Color::WHITE);
    tree.children[0].children[0].spans = Some(vec![SpanDesc {
        text: "Short text".into(),
        color: Some(super::super::desc::Color::WHITE),
        u: true,
        ..Default::default()
    }]);
    tree.children[1].on_click = Some(Handler {
        generation: 2,
        index: 3,
    });
    commit(&mut ui, tree.clone(), &assets, now);
    assert!(!ui.layout_dirty);
    assert_eq!(
        ids,
        ui.nodes.iter().map(|n| n.layout_id).collect::<Vec<_>>()
    );
    assert!(
        ids.into_iter()
            .flatten()
            .all(|id| !ui.layout_tree.dirty(id).unwrap())
    );
    assert_matches_fresh(&mut ui, tree, &mut assets, now);
    ui.pointer_moved(Some((325.0, 10.0)));
    assert_eq!(
        ui.click_target(),
        Some(Handler {
            generation: 2,
            index: 3
        })
    );
}

#[test]
fn text_and_inherited_font_changes_invalidate_only_affected_layout() {
    let (mut ui, mut assets) = setup();
    let now = Instant::now();
    let mut tree = fixture();
    commit(&mut ui, tree.clone(), &assets, now);
    ui.draw(&mut assets, now);
    tree.children[0].children[0].text =
        Some("A much longer sentence that wraps over multiple lines inside the panel.".into());
    tree.children[0].style.font_size = Some(32.0);
    commit(&mut ui, tree.clone(), &assets, now);
    assert!(ui.layout_dirty);
    let sibling = ui.nodes[ui.index["/root/sibling"]].layout_id.unwrap();
    assert!(!ui.layout_tree.dirty(sibling).unwrap());
    assert_matches_fresh(&mut ui, tree, &mut assets, now);
}

#[test]
fn keyed_reorders_insertions_removals_and_kind_changes_match_fresh_layout() {
    let (mut ui, mut assets) = setup();
    let now = Instant::now();
    let mut tree = fixture();
    commit(&mut ui, tree.clone(), &assets, now);
    ui.draw(&mut assets, now);
    let label = ui.nodes[ui.index["/root/panel/label"]].layout_id;
    tree.children.swap(0, 1);
    tree.children[1].children.insert(
        0,
        serde_json::from_value(serde_json::json!({
            "key": "inserted", "t": "text", "text": "Before the label"
        }))
        .unwrap(),
    );
    tree.children[1].children[1].text =
        Some("Updated after moving to a different arena index".into());
    commit(&mut ui, tree.clone(), &assets, now);
    assert_eq!(ui.nodes[ui.index["/root/panel/label"]].layout_id, label);
    assert_matches_fresh(&mut ui, tree.clone(), &mut assets, now);

    tree.children.remove(0);
    tree.children[0].children[1].t = Some(NodeKind::Input);
    tree.children[0].children[1].value = Some(serde_json::json!("replacement"));
    commit(&mut ui, tree.clone(), &assets, now);
    assert_ne!(ui.nodes[ui.index["/root/panel/label"]].layout_id, label);
    assert_eq!(ui.layout_tree.total_node_count(), ui.nodes.len());
    assert_matches_fresh(&mut ui, tree, &mut assets, now);
}

#[test]
fn native_input_edits_and_surface_resize_invalidate_measurements() {
    let (mut ui, mut assets) = setup();
    let now = Instant::now();
    let mut tree = fixture();
    commit(&mut ui, tree.clone(), &assets, now);
    ui.draw(&mut assets, now);
    ui.type_text(" with a longer value");
    tree.children[0].children[1].value = Some(serde_json::json!("hi with a longer value"));
    assert_matches_fresh(&mut ui, tree.clone(), &mut assets, now);
    commit(&mut ui, tree.clone(), &assets, now);
    assert!(!ui.layout_dirty);
    ui.set_surface_size(1920.0, 1080.0);
    assert!(ui.layout_dirty);
    assert_matches_fresh(&mut ui, tree, &mut assets, now);
}

#[test]
fn exit_ghosts_keep_frozen_rectangles_outside_layout_tree() {
    let (mut ui, mut assets) = setup();
    let now = Instant::now();
    let mut tree = fixture();
    tree.children[0].exit =
        Some(serde_json::from_value(serde_json::json!({"dur": 1, "opacity": 0})).unwrap());
    commit(&mut ui, tree.clone(), &assets, now);
    ui.draw(&mut assets, now);
    let old_rect = ui.nodes[ui.index["/root/panel"]].rect;
    tree.children.remove(0);
    ui.commit(tree.clone(), false, &HashMap::new(), &assets, now);
    assert_eq!(ui.layout_tree.total_node_count(), 2);
    assert_matches_fresh(&mut ui, tree.clone(), &mut assets, now);
    let ghost = ui.nodes.iter().find(|n| n.ghost.is_some()).unwrap();
    assert_eq!(ghost.rect, old_rect);
    assert!(ghost.layout_id.is_none());
    ui.commit(tree, false, &HashMap::new(), &assets, now);
    assert_eq!(ui.layout_tree.total_node_count(), 2);
    assert!(!ui.layout_dirty);
}

#[test]
fn tooltip_hover_updates_native_text_without_replacing_the_tree() {
    let (mut ui, mut assets) = setup();
    let now = Instant::now();
    let mut tree = fixture();
    tree.children[0].children[1].focus.autofocus = false;
    tree.children[0].tooltip = Some("Saved dialogue".into());
    tree.children[1].tooltip = Some("Another slot".into());
    tree.children.push(
        serde_json::from_value(serde_json::json!({
            "key": "tooltip", "t": "text", "tooltipText": true,
            "style": { "position": "absolute", "bottom": 0, "maxWidth": 800 }
        }))
        .unwrap(),
    );
    commit(&mut ui, tree.clone(), &assets, now);
    ui.draw(&mut assets, now);
    let indices = ui.index.clone();
    let ids: Vec<_> = ui.nodes.iter().map(|n| n.layout_id).collect();
    let tip_index = ui.index["/root/tooltip"];
    assert!(ui.nodes[tip_index].tooltip_hidden);

    for (position, expected) in [
        ((10.0, 10.0), Some("Saved dialogue")),
        ((325.0, 10.0), Some("Another slot")),
        ((800.0, 400.0), None),
    ] {
        let (_, events) = ui.pointer_moved(Some(position));
        assert_eq!(
            events,
            vec![super::super::InputEvent::Tooltip(
                expected.map(str::to_owned)
            )]
        );
        assert_eq!(ui.index, indices);
        assert_eq!(
            ui.nodes[tip_index].spans.as_ref().unwrap()[0].text,
            expected.unwrap_or_default()
        );
        assert_eq!(ui.nodes[tip_index].tooltip_hidden, expected.is_none());
        let slot = ui.nodes[ui.index["/root/panel"]].layout_id.unwrap();
        assert!(!ui.layout_tree.dirty(slot).unwrap());
        ui.draw(&mut assets, now);
        assert_eq!(
            ids,
            ui.nodes.iter().map(|n| n.layout_id).collect::<Vec<_>>()
        );
        assert_eq!(ui.pointer_moved(Some(position)).1, []);
        assert!(!ui.layout_dirty);
    }
    ui.pointer_moved(Some((10.0, 10.0)));
    ui.draw(&mut assets, now);
    commit(&mut ui, tree, &assets, now);
    assert_eq!(
        ui.nodes[tip_index].spans.as_ref().unwrap()[0].text,
        "Saved dialogue"
    );
    assert!(!ui.layout_dirty);
}
