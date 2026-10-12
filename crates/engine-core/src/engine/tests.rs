use super::*;

mod frame_events;
mod notifications;

#[test]
#[ignore = "Requires a native GPU adapter"]
fn cleanup_deadlines_stop_for_empty_and_retained_scenes_and_resume_for_preloads() {
    const CHILD: &str = "DEFLORTA_CLEANUP_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let directory =
            std::env::temp_dir().join(format!("deflorta-cleanup-test-{}", std::process::id()));
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "engine::tests::cleanup_deadlines_stop_for_empty_and_retained_scenes_and_resume_for_preloads", "--ignored"])
            .env(CHILD, "1")
            .env("XDG_DATA_HOME", &directory)
            .status().unwrap();
        let _ = std::fs::remove_dir_all(directory);
        assert!(status.success());
        return;
    }
    let files = crate::GameFiles::open(&crate::workspace_dir().join("game")).unwrap();
    let host = ScriptHost::new(files.clone()).unwrap();
    let ui = Ui::new(crate::ui::text::TextSystem::new(&files));
    let mut engine = Engine::new(host, Assets::new(files), ui, None);
    let mut renderer = pollster::block_on(Renderer::offscreen(32, 32)).unwrap();
    let now = Instant::now();
    assert_eq!(engine.next_resource_cleanup(), None);
    engine.collect_unused_resources(&mut renderer, now);
    assert_eq!(engine.next_resource_cleanup(), None);
    engine.assets.request("missing.png");
    engine.ui.commit(
        NodeDesc {
            t: Some(crate::ui::desc::NodeKind::Image),
            src: Some("missing.png".into()),
            ..Default::default()
        },
        true,
        &HashMap::new(),
        &engine.assets,
        now,
    );
    engine.collect_unused_resources(&mut renderer, now);
    assert_eq!(engine.next_resource_cleanup(), None);
    engine.ui.commit(
        NodeDesc::default(),
        true,
        &HashMap::new(),
        &engine.assets,
        now,
    );
    engine.collect_unused_resources(&mut renderer, now);
    assert_eq!(
        engine.next_resource_cleanup(),
        Some(now + crate::assets::ASSET_IDLE_LIMIT)
    );
    engine.collect_unused_resources(&mut renderer, now + crate::assets::ASSET_IDLE_LIMIT);
    assert_eq!(engine.next_resource_cleanup(), None);

    // Upload a real texture, then remove its CPU cache so only the renderer's
    // deadline can wake the idle scene.
    let source = "images/masks/clouds.png";
    engine.assets.request(source);
    let deadline = Instant::now() + Duration::from_secs(5);
    while !engine.assets.is_settled(source) {
        assert!(Instant::now() < deadline, "image decoding timed out");
        engine.assets.poll();
        std::thread::sleep(Duration::from_millis(1));
    }
    let now = Instant::now();
    engine.ui.commit(
        NodeDesc {
            t: Some(crate::ui::desc::NodeKind::Image),
            src: Some(source.into()),
            ..Default::default()
        },
        true,
        &HashMap::new(),
        &engine.assets,
        now,
    );
    let items = engine.ui.draw(&mut engine.assets, now);
    renderer
        .render(
            items,
            &mut engine.ui,
            &mut engine.assets,
            Color::WHITE,
            #[cfg(feature = "dev-console")]
            None,
        )
        .unwrap();
    let now = Instant::now();
    engine.collect_unused_resources(&mut renderer, now);
    assert_eq!(engine.next_resource_cleanup(), None);
    engine.assets.forget(source);
    engine.ui.commit(
        NodeDesc::default(),
        true,
        &HashMap::new(),
        &engine.assets,
        now,
    );
    engine.collect_unused_resources(&mut renderer, now);
    assert_eq!(
        engine.next_resource_cleanup(),
        Some(now + crate::assets::ASSET_IDLE_LIMIT)
    );
    engine.collect_unused_resources(&mut renderer, now + crate::assets::ASSET_IDLE_LIMIT);
    assert_eq!(engine.next_resource_cleanup(), None);
}

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
    ScriptHost::set_data_dir(crate::data_dir().join("deflorta/runtime-regressions"));
    host.run_main().unwrap();
    let ui = Ui::new(crate::ui::text::TextSystem::new(&files));
    let mut engine = Engine::new(host, Assets::new(files), ui, None);
    engine.boot();
    engine.take_requests();

    check_loading_deadlines(&mut engine);
    check_instant_dialogue(&mut engine);
    check_shutdown_persistence(&mut engine);
}

fn check_loading_deadlines(engine: &mut Engine) {
    assert!(engine.next_loading_deadline().is_none());
    let since = Instant::now();
    engine.pending = Some(PendingTree {
        tree: NodeDesc {
            t: Some(crate::ui::desc::NodeKind::Image),
            src: Some("not-requested.png".into()),
            ..NodeDesc::default()
        },
        generation: 0,
        instant: true,
        exits: HashMap::new(),
        since,
    });
    assert_eq!(
        engine.next_loading_deadline(),
        Some(since + IMAGE_WAIT_LIMIT)
    );
    engine.try_commit();
    assert!(engine.pending.is_some());
    engine.pending.as_mut().unwrap().since = since.checked_sub(IMAGE_WAIT_LIMIT).unwrap();
    engine.poll();
    assert!(
        engine.pending.is_none(),
        "a stalled decoder must not hold the tree indefinitely"
    );
    engine.capture = Some(CaptureTiming::Settling(since));
    assert_eq!(engine.next_loading_deadline(), Some(since + SETTLE_LIMIT));
    engine.poll();
    assert!(engine.capture == Some(CaptureTiming::Now));
    engine.set_thumbnail(None);
    assert!(engine.next_loading_deadline().is_none());
    engine.take_requests();
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
    ScriptHost::flush_storage().unwrap();
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
    ScriptHost::flush_storage().unwrap();
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

#[test]
fn storage_cache_and_background_failures_reach_the_runtime() {
    let name = "engine::tests::storage_cache_and_background_failures_reach_the_runtime";
    if std::env::var_os("DEFLORTA_STORAGE_TEST_CHILD").is_none() {
        let temp =
            std::env::temp_dir().join(format!("deflorta-storage-runtime-{}", std::process::id()));
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name])
            .env("DEFLORTA_STORAGE_TEST_CHILD", "1")
            .env("XDG_DATA_HOME", &temp)
            .status()
            .unwrap();
        let _ = std::fs::remove_dir_all(temp);
        assert!(status.success());
        return;
    }
    let directory = crate::data_dir().join("deflorta/storage-worker");
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("value.json");
    std::fs::write(&path, r#"{"version":"old"}"#).unwrap();
    let files =
        crate::GameFiles::open(&crate::workspace_dir().join("tests/storage-worker")).unwrap();
    let mut host = ScriptHost::new(files.clone()).unwrap();
    host.run_main().unwrap();
    let ui = Ui::new(crate::ui::text::TextSystem::new(&files));
    let mut engine = Engine::new(host, Assets::new(files), ui, None);
    engine.boot();
    let snapshot = |engine: &Engine| {
        serde_json::from_str::<serde_json::Value>(&engine.config().title).unwrap()
    };
    assert_eq!(snapshot(&engine)["value"]["version"], "old");
    assert!(snapshot(&engine).get("error").is_none());
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    key(&mut engine, "write");
    assert_eq!(snapshot(&engine)["value"]["version"], "new");
    ScriptHost::flush_storage().unwrap();
    engine.poll();
    let result = snapshot(&engine);
    assert_eq!(result["value"]["version"], "old");
    assert!(result["error"].as_str().unwrap().contains("value.json"));
    assert!(!directory.join("transient.json").exists());
    std::fs::remove_dir(&path).unwrap();
    key(&mut engine, "write");
    engine.flush_storage();
    assert_eq!(snapshot(&engine)["value"]["version"], "new");
    let persisted: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(persisted["version"], "new");
}
