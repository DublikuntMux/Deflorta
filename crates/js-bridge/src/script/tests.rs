use super::*;
use deflorta_common::desc::NodeDesc;

fn compiled_fixture(name: &str) -> (tempfile::TempDir, GameFiles) {
    let root = crate::workspace_dir().join("tests").join(name);
    let scripts = tempfile::tempdir().unwrap();
    let source = std::fs::read_to_string(root.join("main.js")).unwrap();
    let compiled = deflorta_script_build::compile_jsx("main.js", &source).unwrap();
    std::fs::write(scripts.path().join("main.js"), compiled.as_bytes()).unwrap();
    let files = GameFiles::with_scripts(&root, scripts.path()).unwrap();
    (scripts, files)
}

fn runtime_key(host: &mut ScriptHost, key: &str) -> Vec<Command> {
    host.dispatch(&Event::Key {
        key,
        down: true,
        repeat: false,
        ctrl: false,
        shift: false,
        alt: false,
        revealing: false,
    })
    .unwrap();
    ScriptHost::take_commands()
}

fn runtime_state(host: &mut ScriptHost) -> serde_json::Value {
    runtime_key(host, "state")
        .into_iter()
        .find_map(|command| match command {
            Command::Voice { file: Some(text) } => Some(serde_json::from_str(&text).unwrap()),
            _ => None,
        })
        .unwrap()
}

fn scheduled_timer(commands: &[Command]) -> u64 {
    commands
        .iter()
        .find_map(|command| match command {
            Command::SetTimer { id, .. } => Some(*id),
            _ => None,
        })
        .expect("automatic progression must schedule a timer")
}

fn committed_tree(commands: &[Command]) -> &NodeDesc {
    commands
        .iter()
        .rev()
        .find_map(|command| match command {
            Command::Commit(commit) => Some(&commit.tree),
            _ => None,
        })
        .expect("expected a UI commit")
}

fn tree_text(tree: &NodeDesc) -> String {
    let mut text = tree.text.clone().unwrap_or_default();
    if let Some(spans) = &tree.spans {
        for span in spans {
            text.push_str(&span.text);
        }
    }
    for child in &tree.children {
        text.push_str(&tree_text(child));
    }
    text
}

fn keyed_node<'a>(tree: &'a NodeDesc, key: &str) -> Option<&'a NodeDesc> {
    if tree.key.as_deref() == Some(key) {
        return Some(tree);
    }
    tree.children
        .iter()
        .find_map(|child| keyed_node(child, key))
}

#[test]
fn story_lifecycle_timers_and_active_translations_regressions() {
    let name = "script::tests::story_lifecycle_timers_and_active_translations_regressions";
    if std::env::var_os("DEFLORTA_STORY_TEST_CHILD").is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name])
            .env("DEFLORTA_STORY_TEST_CHILD", "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let files = GameFiles::open(&crate::workspace_dir().join("tests/runtime-regressions")).unwrap();
    let mut host = ScriptHost::new(files).unwrap();
    let directory = std::env::temp_dir().join(format!("deflorta-story-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    ScriptHost::set_data_dir(directory.clone());
    host.run_main().unwrap();
    drop(ScriptHost::take_commands());
    host.dispatch(&Event::Boot).unwrap();
    drop(ScriptHost::take_commands());

    check_checkpoint_cancellation(&mut host);
    check_modal_progression(&mut host);
    check_active_translations(&mut host);
    drop(host);
    std::fs::remove_dir_all(directory).unwrap();
}

fn check_checkpoint_cancellation(host: &mut ScriptHost) {
    for lifecycle in ["load", "start", "end", "rollback-test", "error-test"] {
        runtime_key(host, "stop-auto");
        runtime_key(host, "start");
        if lifecycle == "rollback-test" {
            runtime_key(host, "advance-test");
        }
        runtime_key(host, "save");
        runtime_key(host, "auto");
        host.dispatch(&Event::Revealed).unwrap();
        let timer = scheduled_timer(&ScriptHost::take_commands());
        runtime_key(host, "stop-auto");
        let commands = runtime_key(host, lifecycle);
        // Disabling Auto has already cleared the timer. Abandoning the
        // checkpoint must still stop its voice and discard the intent.
        assert!(
            commands
                .iter()
                .any(|c| matches!(c, Command::Voice { file: None })),
            "{lifecycle}"
        );
        let before = runtime_state(host);
        host.dispatch(&Event::Timer { id: timer }).unwrap();
        drop(ScriptHost::take_commands());
        assert_eq!(runtime_state(host), before, "stale timer after {lifecycle}");
    }
    runtime_key(host, "start");
    runtime_key(host, "auto");
    host.dispatch(&Event::Revealed).unwrap();
    let timer = scheduled_timer(&ScriptHost::take_commands());
    let commands = runtime_key(host, "start");
    assert!(
        commands
            .iter()
            .any(|c| matches!(c, Command::ClearTimer { id } if *id == timer))
    );
    host.dispatch(&Event::Timer { id: timer }).unwrap();
    drop(ScriptHost::take_commands());
    assert_eq!(runtime_state(host)["line"], "first");
}

fn check_modal_progression(host: &mut ScriptHost) {
    for modal in ["game_menu", "history", "confirm"] {
        runtime_key(host, "start");
        runtime_key(host, "auto");
        host.dispatch(&Event::Revealed).unwrap();
        let timer = scheduled_timer(&ScriptHost::take_commands());
        let commands = runtime_key(host, modal);
        assert!(
            commands
                .iter()
                .any(|c| matches!(c, Command::ClearTimer { id } if *id == timer))
        );
        host.dispatch(&Event::Timer { id: timer }).unwrap();
        drop(ScriptHost::take_commands());
        let state = runtime_state(host);
        assert_eq!(state["line"], "first");
        assert_eq!(state["blocked"], true);
        let commands = runtime_key(host, &format!("close-{modal}"));
        let resumed = scheduled_timer(&commands);
        host.dispatch(&Event::Timer { id: resumed }).unwrap();
        drop(ScriptHost::take_commands());
        assert_eq!(runtime_state(host)["line"], "second");
    }
    runtime_key(host, "stop-auto");
    runtime_key(host, "start");
    let commands = runtime_key(host, "skip");
    let skip = scheduled_timer(&commands);
    let commands = runtime_key(host, "history");
    assert!(
        commands
            .iter()
            .any(|c| matches!(c, Command::ClearTimer { id } if *id == skip))
    );
    host.dispatch(&Event::Timer { id: skip }).unwrap();
    drop(ScriptHost::take_commands());
    assert_eq!(runtime_state(host)["line"], "second");
    let commands = runtime_key(host, "close-history");
    let resumed = scheduled_timer(&commands);
    host.dispatch(&Event::Timer { id: resumed }).unwrap();
    drop(ScriptHost::take_commands());
    assert_eq!(runtime_state(host)["line"], "third");
    runtime_key(host, "stop-skip");
}

fn check_active_translations(host: &mut ScriptHost) {
    runtime_key(host, "translated");
    let commands = runtime_key(host, "uk");
    assert!(tree_text(committed_tree(&commands)).contains("Привіт"));
    runtime_key(host, "save");
    assert_eq!(runtime_state(host)["preview"], "Привіт");
    let commands = runtime_key(host, "source");
    assert!(tree_text(committed_tree(&commands)).contains("Hello"));
    runtime_key(host, "save");
    assert_eq!(runtime_state(host)["preview"], "Hello");

    runtime_key(host, "choices");
    let commands = runtime_key(host, "uk");
    let tree = committed_tree(&commands);
    let text = tree_text(tree);
    assert!(text.contains("Питання") && text.contains("Ліворуч"));
    let choice = keyed_node(tree, "choice-0").unwrap().on_click.unwrap();
    host.dispatch(&Event::Click {
        handler: Some(choice),
        button: "left",
        revealing: false,
    })
    .unwrap();
    drop(ScriptHost::take_commands());
    assert_eq!(runtime_state(host)["choice"], "left");

    let commands = runtime_key(host, "input");
    let input = keyed_node(committed_tree(&commands), "answer")
        .unwrap()
        .on_input
        .unwrap();
    host.dispatch(&Event::Handler {
        handler: input,
        value: Some(HandlerValue::Text("Bob".into())),
    })
    .unwrap();
    drop(ScriptHost::take_commands());
    let commands = runtime_key(host, "source");
    assert!(tree_text(committed_tree(&commands)).contains("Question"));
    assert_eq!(runtime_state(host)["input"], "Bob");
}

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
fn self_voicing_toggle_works_in_modal_screens_and_persists() {
    if std::env::var_os("DEFLORTA_ACCESSIBILITY_TEST_CHILD").is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "script::tests::self_voicing_toggle_works_in_modal_screens_and_persists",
            ])
            .env("DEFLORTA_ACCESSIBILITY_TEST_CHILD", "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let (_scripts, files) = compiled_fixture("accessibility");
    let mut host = ScriptHost::new(files).unwrap();
    host.run_main().unwrap();
    let directory =
        std::env::temp_dir().join(format!("deflorta-accessibility-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    ScriptHost::set_data_dir(directory.clone());
    drop(ScriptHost::take_commands());
    host.dispatch(&Event::Boot).unwrap();
    let commands = ScriptHost::take_commands();
    assert!(
        commands
            .iter()
            .any(|c| matches!(c, Command::SelfVoicing { on: false }))
    );
    let commit = commands
        .iter()
        .find_map(|c| match c {
            Command::Commit(commit) => Some(commit),
            _ => None,
        })
        .unwrap();
    let controls = commit
        .tree
        .children
        .iter()
        .find(|node| node.key.as_deref() == Some("screen:accessibility-test"))
        .unwrap();
    assert!(controls.accessibility.modal);
    assert_eq!(
        controls.children[0].children[0]
            .accessibility
            .alt
            .as_deref(),
        Some("Save game")
    );
    assert_eq!(
        controls.children[0].children[1]
            .accessibility
            .label
            .as_deref(),
        Some("Your name")
    );

    let key = |repeat| Event::Key {
        key: "F6",
        down: true,
        repeat,
        ctrl: false,
        shift: false,
        alt: false,
        revealing: false,
    };
    host.dispatch(&key(false)).unwrap();
    assert!(
        ScriptHost::take_commands()
            .iter()
            .any(|c| matches!(c, Command::SelfVoicing { on: true }))
    );
    ScriptHost::flush_storage().unwrap();
    let prefs: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(directory.join("prefs.json")).unwrap())
            .unwrap();
    assert_eq!(prefs["selfVoicing"], true);
    host.dispatch(&key(true)).unwrap();
    assert!(ScriptHost::take_commands().is_empty());
    host.dispatch(&key(false)).unwrap();
    assert!(
        ScriptHost::take_commands()
            .iter()
            .any(|c| matches!(c, Command::SelfVoicing { on: false }))
    );
    ScriptHost::flush_storage().unwrap();
    let prefs: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(directory.join("prefs.json")).unwrap())
            .unwrap();
    assert_eq!(prefs["selfVoicing"], false);
    drop(host);
    std::fs::remove_dir_all(directory).unwrap();
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
    let (_scripts, files) = compiled_fixture("ui-hover");
    let mut host = ScriptHost::new(files).unwrap();
    host.run_main().unwrap();
    drop(ScriptHost::take_commands());
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
    drop(ScriptHost::take_commands());
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
    drop(ScriptHost::take_commands());
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
    let files = GameFiles::open(&crate::workspace_dir().join("tests/bridge")).unwrap();
    let mut host = ScriptHost::new(files).unwrap();
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
    check_grouped_properties(&first.tree);
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
        HandlerValue::Number(-0.0),
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

fn check_grouped_properties(tree: &NodeDesc) {
    assert!(!tree.is_focusable());
    assert!(tree.focus.autofocus && tree.accessibility.live && tree.accessibility.modal);
    assert_eq!(tree.accessibility.label.as_deref(), Some("Accessible tree"));
    let span = &tree.spans.as_ref().unwrap()[0];
    assert!(span.emphasis.b && span.emphasis.i && span.u && span.s);
    assert!(span.reveal.click && span.reveal.fast);
    assert_eq!(span.reveal.wait, Some(0.25));
}
