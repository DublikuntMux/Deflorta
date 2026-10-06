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

#[cfg(all(debug_assertions, feature = "dev-console"))]
#[test]
fn console_evaluates_live_state_and_recovers_after_errors() {
    // SpiderMonkey cannot be reinitialized in the same process.
    if std::env::var_os("DEFLORTA_CONSOLE_TEST_CHILD").is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "script::tests::console_evaluates_live_state_and_recovers_after_errors",
            ])
            .env("DEFLORTA_CONSOLE_TEST_CHILD", "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let files = GameFiles::open(&crate::workspace_dir().join("tests/ui-hover")).unwrap();
    let mut host = ScriptHost::new(files.clone()).unwrap();
    host.run_main().unwrap();
    host.enable_console().unwrap();
    let ui = crate::ui::Ui::new(crate::ui::text::TextSystem::new(&files));
    let mut engine = crate::engine::Engine::new(host, crate::assets::Assets::new(files), ui, None);
    engine.take_requests();

    assert_eq!(engine.evaluate_console("1 + 2").unwrap(), "3");
    assert_eq!(engine.evaluate_console("undefined").unwrap(), "undefined");
    assert_eq!(
        engine
            .evaluate_console("({answer: 42, text: '你好'})")
            .unwrap(),
        "{\"answer\":42,\"text\":\"你好\"}"
    );
    engine.evaluate_console("let consoleProbe = 7").unwrap();
    assert_eq!(engine.evaluate_console("consoleProbe * 6").unwrap(), "42");
    engine
        .evaluate_console("deflorta.store.consoleProbe = 'live'")
        .unwrap();
    assert_eq!(
        engine
            .evaluate_console("deflorta.store.consoleProbe")
            .unwrap(),
        "live"
    );

    // Native commands and promise continuations both update the real engine.
    engine
        .evaluate_console(
            "Promise.resolve().then(() => deflorta.configure({title: 'Console title'}))",
        )
        .unwrap();
    assert_eq!(engine.config().title, "Console title");
    assert_eq!(
        engine.take_requests().title.as_deref(),
        Some("Console title")
    );
    engine.evaluate_console("deflorta.screen('console-probe', () => deflorta.text('Live console UI')); deflorta.showScreen('console-probe')").unwrap();
    engine.resize(1280, 720);
    assert!(engine.frame(Instant::now()).iter().any(|item| match item {
        crate::ui::DrawItem::Text(text) => engine.ui.text.entry(&text.id).is_some_and(|entry| {
            entry
                .buffer
                .lines
                .iter()
                .any(|line| line.text() == "Live console UI")
        }),
        crate::ui::DrawItem::Quad(_) => false,
    }));

    let error = engine
        .evaluate_console(
            "deflorta.configure({title: 'Before throw'}); throw new Error('console failure')",
        )
        .unwrap_err()
        .to_string();
    assert!(error.contains("console failure") && error.contains("<console>"));
    assert_eq!(engine.config().title, "Before throw");
    assert!(
        engine
            .evaluate_console("const =")
            .unwrap_err()
            .to_string()
            .contains("SyntaxError")
    );
    assert_eq!(engine.evaluate_console("consoleProbe").unwrap(), "7");
    // Formatting errors (e.g. circular objects or a throwing toJSON) are contained.
    assert_eq!(
        engine
            .evaluate_console("let cycle = {}; cycle.self = cycle; cycle")
            .unwrap(),
        "[object Object]"
    );
    assert_eq!(
        engine
            .evaluate_console("({toJSON() { throw new Error('format'); }})")
            .unwrap(),
        "[object Object]"
    );
    assert_eq!(engine.evaluate_console("6 * 7").unwrap(), "42");
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
    let files = GameFiles::open(&crate::workspace_dir().join("tests/ui-hover")).unwrap();
    let mut host = ScriptHost::new(files).unwrap();
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
