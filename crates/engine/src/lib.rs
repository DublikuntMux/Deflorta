// wgpu's nested backend types need this depth for async Send/Sync checks.
#![recursion_limit = "256"]

#[cfg(target_os = "android")]
mod android_ime;
mod app;
mod assets;
mod audio;
#[cfg(feature = "dev-console")]
mod dev_console;
mod engine;
mod headless;
mod media;
mod render;
mod script;
mod self_voicing;
mod storage;
mod ui;
mod util;
mod video;
mod worker;

use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use log::info;
use winit::event_loop::EventLoop;

pub use deflorta_data::{GameFiles, archive, files, font_families};
pub use script::{BUILTIN_MODULES, GameConfig, is_builtin_module, resolve_specifier};

/// Log filter used when `RUST_LOG` is not set: engine and script messages at
/// info, third-party crates only when something goes wrong.
pub const DEFAULT_LOG_FILTER: &str = "warn,deflorta=info";

pub fn init_logging(default_filter: &str) {
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| default_filter.to_owned());
    #[cfg(feature = "dev-console")]
    dev_console::init_logging(&filter);
    #[cfg(not(all(feature = "dev-console")))]
    pretty_env_logger::formatted_timed_builder()
        .parse_filters(&filter)
        .init();
}

pub fn run(files: GameFiles, test_script: Option<&Path>) -> Result<()> {
    run_with_event_loop(files, test_script, EventLoop::with_user_event())
}

#[cfg(target_os = "android")]
pub fn run_android(
    files: GameFiles,
    app: winit::platform::android::activity::AndroidApp,
) -> Result<()> {
    use winit::platform::android::EventLoopBuilderExtAndroid;

    ANDROID_DATA_DIR
        .set(app.internal_data_path().context("no app data directory")?)
        .map_err(|_| anyhow::anyhow!("Android runtime already initialized"))?;
    let mut builder = EventLoop::with_user_event();
    builder.with_android_app(app);
    run_with_event_loop(files, None, builder)
}

#[cfg(target_os = "android")]
static ANDROID_DATA_DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

fn data_dir() -> std::path::PathBuf {
    #[cfg(target_os = "android")]
    return ANDROID_DATA_DIR
        .get()
        .expect("Android runtime not initialized")
        .clone();
    #[cfg(not(target_os = "android"))]
    dirs::data_dir().unwrap_or_else(|| std::path::PathBuf::from("."))
}

fn run_with_event_loop(
    files: GameFiles,
    test_script: Option<&Path>,
    mut builder: winit::event_loop::EventLoopBuilder<app::AppEvent>,
) -> Result<()> {
    info!(
        "Deflorta {} on {}/{}, game {}, {} mode",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        files.location().display(),
        if test_script.is_some() {
            "headless test"
        } else {
            "windowed"
        },
    );

    let started = Instant::now();
    let mut script = script::ScriptHost::new(files.clone())?;
    script
        .run_main()
        .context("failed to start the game script")?;
    let ui = ui::Ui::new(ui::text::TextSystem::new(&files));
    let assets = assets::Assets::new(files);

    if let Some(test_script) = test_script {
        let engine = engine::Engine::new(script, assets, ui, None);
        info!("Startup took {:.0?}", started.elapsed());
        return headless::run(engine, test_script);
    }

    let engine = engine::Engine::new(script, assets, ui, audio::Audio::new());
    info!("Startup took {:.0?}", started.elapsed());
    let event_loop = builder.build()?;
    let mut app = app::App::new(engine, event_loop.create_proxy());
    event_loop.run_app(&mut app)?;
    app.take_error().map_or_else(|| Ok(()), Err)
}

/// Evaluates the game's scripts without a window; returns their configuration.
///
/// Fails on syntax errors, missing modules and exceptions thrown while the
/// modules load. The script engine can start only once per process.
pub fn boot(files: GameFiles) -> Result<GameConfig> {
    let mut script = script::ScriptHost::new(files)?;
    script.run_main()?;
    let config = script::ScriptHost::take_commands()
        .into_iter()
        .rev()
        .find_map(|command| match command {
            script::Command::Configure(config) => Some(config),
            _ => None,
        })
        .context("the runtime did not configure the game")?;
    Ok(config)
}

#[cfg(test)]
fn workspace_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
