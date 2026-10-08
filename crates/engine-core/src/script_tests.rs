use deflorta_js_bridge::script::*;
use std::time::Instant;

#[cfg(feature = "dev-console")]
fn assert_console_diagnostics(engine: &mut crate::engine::Engine) {
    let assets = engine.loaded_assets();
    assert!(
        assets
            .iter()
            .any(|asset| asset.kind == "JavaScript" && asset.source == "main.js")
    );
    assert!(assets.iter().any(|asset| asset.kind == "Font"));
    assert!(engine.diagnostic_stats().iter().any(|(label, value)| {
        label == "Shaped text buffers" && value.parse::<usize>().unwrap() > 0
    }));
    let tree = engine
        .ui
        .accessibility_update(&engine.config().title.clone());
    let report = crate::dev_console::diagnostics::tree_report(&tree);
    assert!(report.contains("Live console UI"));
    assert_eq!(
        tree,
        engine
            .ui
            .accessibility_update(&engine.config().title.clone())
    );
    assert_eq!(engine.evaluate_console("consoleProbe").unwrap(), "7");
}

#[cfg(feature = "dev-console")]
#[test]
fn console_evaluates_live_state_and_recovers_after_errors() {
    // SpiderMonkey cannot be reinitialized in the same process.
    if std::env::var_os("DEFLORTA_CONSOLE_TEST_CHILD").is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "script_tests::console_evaluates_live_state_and_recovers_after_errors",
            ])
            .env("DEFLORTA_CONSOLE_TEST_CHILD", "1")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let (_scripts, files) = crate::compiled_fixture("ui-hover");
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
    engine.evaluate_console("deflorta.screen('console-probe', () => deflorta.createElement(deflorta.Text, null, 'Live console UI')); deflorta.showScreen('console-probe')").unwrap();
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

    assert_console_diagnostics(&mut engine);

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
