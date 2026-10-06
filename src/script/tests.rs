use super::*;

fn voices() -> Vec<String> {
    ScriptHost::take_commands()
        .into_iter()
        .map(|command| match command {
            Command::Voice { file: Some(file) } => file,
            _ => panic!("expected a voice command"),
        })
        .collect()
}

#[test]
fn native_tooltip_hover_skips_commits_and_custom_tooltips_still_render() {
    // SpiderMonkey can initialize only once per process; isolate this second
    // scripting scenario from the existing bridge test.
    if std::env::var_os("DEFLORTA_TOOLTIP_TEST_CHILD").is_some() {
        check_native_tooltip_hover();
        return;
    }
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "script::tests::native_tooltip_hover_skips_commits_and_custom_tooltips_still_render",
        ])
        .env("DEFLORTA_TOOLTIP_TEST_CHILD", "1")
        .status()
        .unwrap();
    assert!(status.success());
}

fn check_native_tooltip_hover() {
    let game_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/ui-hover");
    let mut host = ScriptHost::new(game_dir).unwrap();
    host.run_main().unwrap();
    ScriptHost::take_commands();
    host.flush().unwrap();
    let initial = ScriptHost::take_commands();
    let commit = initial
        .iter()
        .find_map(|command| match command {
            Command::Commit(commit) => Some(commit),
            _ => None,
        })
        .unwrap();
    let handler = commit
        .tree
        .children
        .iter()
        .find(|node| node.key.as_deref() == Some("hover-probe"))
        .unwrap()
        .on_click
        .unwrap();
    let tooltip = commit
        .tree
        .children
        .iter()
        .find(|node| node.key.as_deref() == Some("tooltip"))
        .unwrap();
    assert!(tooltip.children[0].tooltip_text);

    // Entering another slot, repeating its preview, and leaving it produce
    // no tree commits or screen renders with the default tooltip.
    for text in [
        Some("Saved dialogue"),
        Some("Another slot"),
        Some("Another slot"),
        None,
    ] {
        host.dispatch(&Event::Tooltip {
            text: text.map(str::to_owned),
        })
        .unwrap();
        assert!(ScriptHost::take_commands().is_empty());
    }
    unsafe { jsapi::JS_GC(host.cx(), jsapi::GCReason::API) };
    host.dispatch(&Event::Click {
        handler: Some(handler),
        button: "left",
        revealing: false,
    })
    .unwrap();
    assert_eq!(voices(), ["click"]);

    let key = |key| Event::Key {
        key,
        down: true,
        repeat: false,
        ctrl: false,
        shift: false,
        alt: false,
        revealing: false,
    };
    host.dispatch(&key("custom-tooltip")).unwrap();
    ScriptHost::take_commands();
    host.dispatch(&Event::Tooltip {
        text: Some("Custom preview".into()),
    })
    .unwrap();
    let commands = ScriptHost::take_commands();
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, Command::Commit(commit)
        if commit.tree.children.iter().any(|node| node.text.as_deref() == Some("Custom preview"))))
    );
    host.dispatch(&Event::Tooltip {
        text: Some("Custom preview".into()),
    })
    .unwrap();
    assert!(ScriptHost::take_commands().is_empty());

    host.dispatch(&key("hide-ui")).unwrap();
    ScriptHost::take_commands();
    host.dispatch(&Event::Tooltip {
        text: Some("Changed while hidden".into()),
    })
    .unwrap();
    assert!(ScriptHost::take_commands().is_empty());
    host.dispatch(&key("show-ui")).unwrap();
    assert!(ScriptHost::take_commands().iter().any(|command| matches!(command, Command::Commit(commit)
        if commit.tree.children.iter().any(|node| node.text.as_deref() == Some("Changed while hidden")))));
}

#[test]
fn direct_bridge_preserves_values_and_committed_callbacks() {
    let game_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/bridge");
    let mut host = ScriptHost::new(game_dir).unwrap();
    host.run_main().unwrap();
    let commands = ScriptHost::take_commands();
    let config = commands
        .iter()
        .find_map(|command| match command {
            Command::Configure(config) if config.id == "bridge-test" => Some(config),
            _ => None,
        })
        .unwrap();
    assert_eq!(config.title, "Привіт 🌸");
    assert_eq!(
        config.version,
        Some(serde_json::json!({ "items": [1, null, null] }))
    );
    let commits: Vec<_> = commands
        .into_iter()
        .filter_map(|command| match command {
            Command::Commit(commit) => Some(commit),
            _ => None,
        })
        .collect();
    assert_eq!(commits.len(), 2);
    let first = &commits[0];
    assert!(first.instant);
    assert!(first.tree.text.is_none());
    assert!(first.tree.tooltip.is_none());
    assert!(first.tree.style.opacity.is_none());
    assert_eq!(first.tree.children[0].text.as_deref(), Some("Привіт 🌸"));
    assert!(first.tree.children[0].cps.is_none());
    assert_eq!(
        first.tree.value,
        Some(serde_json::json!({
            "0": "numeric key", "ключ": "значення", "items": [1, null, null],
        }))
    );
    assert!(first.exits["sprite"].is_some());
    assert!(first.exits["deleted"].is_none());
    let old_handler = first.tree.on_click.unwrap();
    let new_handler = commits[1].tree.on_click.unwrap();

    // A pending replacement must not change the callback on the displayed tree.
    // A full GC exercises the roots that retain otherwise unreachable closures.
    unsafe { jsapi::JS_GC(host.cx(), jsapi::GCReason::API) };
    host.dispatch(&Event::Click {
        handler: Some(old_handler),
        button: "left",
        revealing: false,
    })
    .unwrap();
    assert_eq!(voices(), ["first", "microtask", "flush"]);
    ScriptHost::release_handlers(commits[1].generation);
    unsafe { jsapi::JS_GC(host.cx(), jsapi::GCReason::API) };
    host.dispatch(&Event::Click {
        handler: Some(old_handler),
        button: "left",
        revealing: false,
    })
    .unwrap();
    assert_eq!(voices(), ["released", "flush"]);
    host.dispatch(&Event::Click {
        handler: Some(new_handler),
        button: "left",
        revealing: false,
    })
    .unwrap();
    assert_eq!(voices(), ["second", "microtask", "flush"]);

    for value in [
        HandlerValue::Number(0.1),
        HandlerValue::Text("Привіт 🌸".into()),
    ] {
        host.dispatch(&Event::Handler {
            handler: new_handler,
            value: Some(value),
        })
        .unwrap();
        assert_eq!(voices(), ["second", "flush"]);
    }
    host.dispatch(&Event::Key {
        key: "🌸",
        down: true,
        repeat: false,
        ctrl: true,
        shift: false,
        alt: false,
        revealing: false,
    })
    .unwrap();
    assert_eq!(voices(), ["flush"]);
    host.dispatch(&Event::Timer { id: 4_294_967_297 }).unwrap();
    assert_eq!(voices(), ["flush"]);
}
