use super::*;

fn key(engine: &mut Engine, name: &str) {
    engine.key(name, true, false, &KeyModifiers::default());
}

fn state(engine: &mut Engine) -> serde_json::Value {
    key(engine, "state");
    serde_json::from_str(&engine.config().title).unwrap()
}

fn frame(engine: &mut Engine) {
    let now = Instant::now();
    engine.frame(now);
    engine.after_frame(now);
}

fn fire_all(engine: &mut Engine) {
    for (due, _) in &mut engine.timers {
        *due = Instant::now();
    }
    engine.fire_timers();
}

#[test]
fn instant_dialogue_and_shutdown_persistence_regressions() {
    let name = "engine::tests::instant_dialogue_and_shutdown_persistence_regressions";
    if std::env::var_os("DEFLORTA_ENGINE_TEST_CHILD").is_none() {
        let temp = std::env::temp_dir().join(format!("deflorta-engine-{}", std::process::id()));
        for mode in ["request", "close"] {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", name])
                .env("DEFLORTA_ENGINE_TEST_CHILD", mode)
                .env("XDG_DATA_HOME", temp.join(mode))
                .status()
                .unwrap();
            assert!(status.success());
        }
        let _ = std::fs::remove_dir_all(temp);
        return;
    }
    let files =
        crate::GameFiles::open(&crate::workspace_dir().join("tests/runtime-regressions")).unwrap();
    let mut host = ScriptHost::new(files.clone()).unwrap();
    ScriptHost::set_data_dir(
        dirs::data_dir()
            .unwrap()
            .join("deflorta/runtime-regressions"),
    );
    host.run_main().unwrap();
    let ui = Ui::new(crate::ui::text::TextSystem::new(&files));
    let mut engine = Engine::new(host, Assets::new(files), ui, None);
    engine.boot();
    engine.take_requests();

    check_instant_dialogue(&mut engine);
    check_shutdown_persistence(&mut engine);
}

fn check_instant_dialogue(engine: &mut Engine) {
    key(engine, "noWait");
    frame(engine);
    engine.fire_timers();
    assert_eq!(state(engine)["finished"], true);
    frame(engine);
    engine.fire_timers();
    assert_eq!(state(engine)["repeated"], true);

    key(engine, "translatedNoWait");
    frame(engine);
    assert_eq!(engine.timers, [] as [(Instant, u64); 0]);
    key(engine, "uk");
    engine.fire_timers();
    assert_eq!(state(engine)["finished"], true);
    key(engine, "translatedNoWait");
    frame(engine);
    assert_eq!(engine.timers.len(), 1);
    key(engine, "source");
    engine.fire_timers();
    assert_eq!(state(engine)["finished"], false);
    assert_eq!(engine.timers, [] as [(Instant, u64); 0]);

    key(engine, "repeated");
    key(engine, "auto");
    frame(engine);
    assert_eq!(engine.timers.len(), 1);
    key(engine, "save");
    fire_all(engine);
    assert_eq!(state(engine)["first"], true);
    frame(engine);
    assert_eq!(
        engine.timers.len(),
        1,
        "identical consecutive dialogue must complete again"
    );
    fire_all(engine);
    assert_eq!(state(engine)["second"], true);
    key(engine, "load");
    frame(engine);
    assert_eq!(
        engine.timers.len(),
        1,
        "an instant restored checkpoint must emit revealed"
    );
    fire_all(engine);
    assert_eq!(state(engine)["first"], true);
}

fn check_shutdown_persistence(engine: &mut Engine) {
    key(engine, "stop-auto");
    key(engine, "start");
    if std::env::var("DEFLORTA_ENGINE_TEST_CHILD").unwrap() == "close" {
        key(engine, "prepare-quit");
        engine.quit();
    } else {
        key(engine, "quit-test");
    }
    let requests = engine.take_requests();
    assert!(requests.quit && requests.capture);
    let data = &engine.data_dir;
    let read = |name| -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(data.join(name)).unwrap()).unwrap()
    };
    assert_eq!(read("persistent.json")["changed"], "saved on quit");
    assert_eq!(read("quit-count.json"), 1);
    let save = read("save-auto-1.json");
    assert_eq!(save["root"]["label"], "start");
    assert_eq!(save["target"], 0);
    assert!(!read("seen.json").as_object().unwrap().is_empty());
    engine.set_thumbnail(Some(image::RgbaImage::new(1280, 720)));
    assert!(engine.data_dir.join("thumb-auto-1.png").is_file());
    engine.quit();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            &std::fs::read_to_string(engine.data_dir.join("quit-count.json")).unwrap(),
        )
        .unwrap(),
        1
    );
}
