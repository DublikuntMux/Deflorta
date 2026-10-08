use super::*;
use deflorta_common::notification::{NotificationOptions, NotificationState, next_notification_id};

#[test]
fn javascript_notification_handles_control_independent_native_notifications() {
    let name = "engine::tests::notifications::javascript_notification_handles_control_independent_native_notifications";
    if std::env::var_os("DEFLORTA_NOTIFICATION_TEST_CHILD").is_none() {
        let directory = tempfile::tempdir().unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", name])
            .env("DEFLORTA_NOTIFICATION_TEST_CHILD", "1")
            .env("XDG_DATA_HOME", directory.path())
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let files =
        crate::GameFiles::open(&crate::workspace_dir().join("tests/notifications")).unwrap();
    let mut host = ScriptHost::new(files.clone()).unwrap();
    host.run_main().unwrap();
    let ui = Ui::new(crate::ui::text::TextSystem::new(&files));
    let mut engine = Engine::new(host, Assets::new(files), ui, None);
    engine.boot();
    engine.take_requests();
    frame(&mut engine);
    assert!(message(&mut engine, "Preferences"));
    let id = next_notification_id();
    engine.ui.show_notification(
        id,
        "System task".into(),
        NotificationOptions {
            state: NotificationState::Loading,
            duration: None,
        },
        Instant::now(),
    );

    key(&mut engine, "create");
    let created = state(&mut engine)["id"].as_u64().unwrap();
    assert_ne!(created, id);
    frame(&mut engine);
    assert!(message(&mut engine, "Enabling self-voicing…"));
    assert!(engine.ui.next_notification_deadline().is_none());
    key(&mut engine, "second");
    assert_ne!(state(&mut engine)["secondId"].as_u64().unwrap(), created);
    key(&mut engine, "update");
    frame(&mut engine);
    assert!(message(&mut engine, "Still working…"));
    assert!(message(&mut engine, "Saved"));
    key(&mut engine, "invalid");
    assert_eq!(state(&mut engine)["rejected"], 8);
    key(&mut engine, "complete");
    frame(&mut engine);
    assert!(message(&mut engine, "Ready"));
    assert_eq!(state(&mut engine)["id"], created);
    assert!(engine.ui.next_notification_deadline().is_some());

    key(&mut engine, "expire");
    engine.poll();
    key(&mut engine, "afterExpire");
    frame(&mut engine);
    assert!(!message(&mut engine, "Must not return"));
    assert!(message(&mut engine, "Saved"));
    assert!(message(&mut engine, "System task"));
    assert!(engine.ui.next_notification_deadline().is_none());
    key(&mut engine, "create");
    key(&mut engine, "dismiss");
    key(&mut engine, "afterDismiss");
    frame(&mut engine);
    assert!(!message(&mut engine, "Enabling self-voicing…"));
    assert!(!message(&mut engine, "Must not return"));
    assert!(message(&mut engine, "Saved"));
    key(&mut engine, "dismissSecond");
    frame(&mut engine);
    assert!(!message(&mut engine, "Saved"));
    engine.ui.dismiss_notification(id);
    assert!(
        !engine
            .ui
            .is_animating(Instant::now() + Duration::from_secs(60))
    );
    engine.quit();
    engine.flush_storage();
}

fn message(engine: &mut Engine, label: &str) -> bool {
    engine
        .ui
        .accessibility_update("Test")
        .nodes
        .iter()
        .any(|(_, node)| node.label() == Some(label))
}
