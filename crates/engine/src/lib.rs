//! Deflorta — a visual novel engine scripted in JavaScript.
//!
//! The engine runs a game from a project directory (development) or from a
//! `.dm` archive (published games). The `deflorta` CLI and the game launcher
//! are thin front ends over [`run`].

// wgpu's nested backend types need this depth for async Send/Sync checks.
#![recursion_limit = "256"]

mod app;
pub mod archive;
mod assets;
mod audio;
#[cfg(all(debug_assertions, feature = "dev-console"))]
mod dev_console;
mod engine;
pub mod files;
mod headless;
mod media;
mod render;
mod script;
mod ui;
mod util;
mod video;

use std::path::Path;
use std::time::Instant;

use anyhow::{Context, Result};
use log::info;
use winit::event_loop::EventLoop;

pub use files::GameFiles;
pub use script::{BUILTIN_MODULES, GameConfig, is_builtin_module, resolve_specifier};
pub use ui::text::font_families;

/// TypeScript declarations for the scripting API, for editor completion and
/// type checking of game code.
pub const TYPE_DECLARATIONS: &str = include_str!("../runtime/deflorta.d.ts");

/// Log filter used when `RUST_LOG` is not set: engine and script messages at
/// info, third-party crates only when something goes wrong.
pub const DEFAULT_LOG_FILTER: &str = "warn,deflorta=info";

/// Logs go to stderr with timestamps. `RUST_LOG=deflorta=debug` (or `trace`)
/// shows more detail; `RUST_LOG=debug` includes wgpu, winit and other crates.
pub fn init_logging(default_filter: &str) {
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| default_filter.to_owned());
    #[cfg(all(debug_assertions, feature = "dev-console"))]
    dev_console::init_logging(&filter);
    #[cfg(not(all(debug_assertions, feature = "dev-console")))]
    pretty_env_logger::formatted_timed_builder()
        .parse_filters(&filter)
        .init();
}

/// Runs a game in a window, or headless with a test script (see `headless.rs`).
pub fn run(files: GameFiles, test_script: Option<&Path>) -> Result<()> {
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
    let mut app = app::App::new(engine);
    EventLoop::new()?.run_app(&mut app)?;
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

/// The repository root, where the demo game and test fixtures live.
#[cfg(test)]
fn workspace_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
