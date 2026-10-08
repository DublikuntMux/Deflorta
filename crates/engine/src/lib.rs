mod app;

#[cfg(target_os = "android")]
mod android_ime;

use deflorta_assets::{assets, audio};
use deflorta_engine_core::{engine, render, self_voicing};
use deflorta_headless::headless;
use deflorta_js_bridge::script;
use deflorta_ui::ui;

#[cfg(target_os = "android")]
use deflorta_common::ANDROID_DATA_DIR;
#[cfg(feature = "dev-console")]
use deflorta_engine_core::dev_console;

use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use log::info;
use winit::event_loop::EventLoop;

pub use deflorta_assets::{GameFiles, archive, files, font_families};
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

///
/// # Errors
///
/// Returns an error if game initialization, event-loop creation, or rendering fails.
pub fn run(files: GameFiles, test_script: Option<&Path>) -> Result<()> {
    run_with_event_loop(files, test_script, EventLoop::with_user_event())
}

#[cfg(target_os = "android")]
/// Runs a game with the Android activity's event loop and data directory.
///
/// # Errors
///
/// Returns an error if the activity has no data directory, the runtime is already
/// initialized, or game initialization, event-loop creation, or rendering fails.
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
///
/// # Errors
///
/// Returns an error for missing modules, syntax errors, script exceptions, or a missing game configuration.
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
