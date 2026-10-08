use super::*;

#[test]
fn presentation_events_are_opt_in_monotonic_and_reset_on_disable() {
    let name = "engine::tests::frame_events::presentation_events_are_opt_in_monotonic_and_reset_on_disable";
    if std::env::var_os("DEFLORTA_FRAME_TEST_CHILD").is_none() {
        let temp = std::env::temp_dir().join(format!("deflorta-frames-{}", std::process::id()));
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name])
            .env("DEFLORTA_FRAME_TEST_CHILD", "1")
            .env("XDG_DATA_HOME", &temp)
            .status()
            .unwrap();
        assert!(status.success());
        let _ = std::fs::remove_dir_all(temp);
        return;
    }
    let files =
        crate::GameFiles::open(&crate::workspace_dir().join("tests/benchmark-frame-events"))
            .unwrap();
    let mut host = ScriptHost::new(files.clone()).unwrap();
    host.run_main().unwrap();
    let ui = Ui::new(crate::ui::text::TextSystem::new(&files));
    let mut engine = Engine::new(host, Assets::new(files), ui, None);
    engine.boot();
    engine.take_requests();
    let start = Instant::now();
    engine.presented_frame(start);
    engine.presented_frame(start + Duration::from_millis(10));
    assert_eq!(state(&mut engine), serde_json::json!([]));

    key(&mut engine, "enable");
    assert!(engine.after_frame(start));
    // Simulated headless draws do not count as window presentations.
    frame(&mut engine);
    frame(&mut engine);
    assert_eq!(state(&mut engine), serde_json::json!([]));
    engine.presented_frame(start);
    engine.presented_frame(start + Duration::from_millis(16));
    engine.presented_frame(start + Duration::from_millis(36));
    assert_eq!(state(&mut engine), serde_json::json!([16, 20]));

    key(&mut engine, "disable");
    engine.presented_frame(start + Duration::from_secs(1));
    assert_eq!(state(&mut engine), serde_json::json!([16, 20]));
    key(&mut engine, "enable");
    engine.presented_frame(start + Duration::from_secs(2));
    assert_eq!(state(&mut engine), serde_json::json!([16, 20]));
    engine.presented_frame(start + Duration::from_secs(2) + Duration::from_micros(12_500));
    assert_eq!(state(&mut engine), serde_json::json!([16, 20, 12.5]));
}
