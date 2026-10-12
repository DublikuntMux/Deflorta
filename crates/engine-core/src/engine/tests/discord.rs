use super::*;

#[test]
fn activity_updates_survive_configuration_changes_and_clear_explicitly() {
    const CHILD: &str = "DEFLORTA_DISCORD_ENGINE_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let data = tempfile::tempdir().unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "engine::tests::discord::activity_updates_survive_configuration_changes_and_clear_explicitly"])
            .env(CHILD, "1")
            .env("XDG_DATA_HOME", data.path())
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("main.js"),
        r#"
        import { configure } from "deflorta";
        configure({ id: "discord-test", discord: { applicationId: "1234567890123456789" } });
    "#,
    )
    .unwrap();
    let files = crate::GameFiles::open(directory.path()).unwrap();
    let mut script = ScriptHost::new(files.clone()).unwrap();
    script.run_main().unwrap();
    let ui = Ui::new(crate::ui::text::TextSystem::new(&files));
    let mut engine = Engine::new(script, Assets::new(files), ui, None);
    assert_eq!(engine.discord_activity(), &Some(DiscordActivity::default()));
    let activity = DiscordActivity {
        details: Some("Chapter one".into()),
        ..Default::default()
    };
    engine.apply(vec![Command::DiscordActivity(Some(activity.clone()))]);
    let mut config = engine.config().clone();
    config.title = "Updated title".into();
    config.discord = None;
    engine.apply(vec![Command::Configure(config.clone())]);
    assert_eq!(engine.discord_activity(), &Some(activity));
    engine.apply(vec![Command::DiscordActivity(None)]);
    assert!(engine.discord_activity().is_none());
    config.discord = Some(deflorta_common::discord::DiscordConfig {
        application_id: "1234567890123456789".into(),
    });
    engine.apply(vec![Command::Configure(config)]);
    assert!(engine.discord_activity().is_none());
}
