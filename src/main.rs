//! Deflorta — a visual novel engine scripted in JavaScript.

// wgpu's nested backend types need this depth for async Send/Sync checks.
#![recursion_limit = "256"]

mod app;
mod assets;
mod audio;
mod engine;
mod headless;
mod render;
mod script;
mod ui;
mod util;
mod video;

use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use log::{error, info};
use winit::event_loop::EventLoop;

const USAGE: &str = "usage: deflorta [GAME_DIR] [--test SCRIPT.json]";

/// Log filter used when `RUST_LOG` is not set: engine and script messages at
/// info, third-party crates only when something goes wrong.
const DEFAULT_LOG_FILTER: &str = "warn,deflorta=info";

/// Logs go to stderr with timestamps. `RUST_LOG=deflorta=debug` (or `trace`)
/// shows more detail; `RUST_LOG=debug` includes wgpu, winit and other crates.
fn init_logging() {
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| DEFAULT_LOG_FILTER.to_owned());
    pretty_env_logger::formatted_timed_builder()
        .parse_filters(&filter)
        .init();
}

fn run() -> Result<()> {
    let mut game_dir = None;
    let mut test_script = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--test") => test_script = Some(PathBuf::from(args.next().context(USAGE)?)),
            Some("-h" | "--help") => {
                println!("{USAGE}");
                return Ok(());
            }
            _ if game_dir.is_none() => game_dir = Some(PathBuf::from(arg)),
            _ => bail!(USAGE),
        }
    }
    let game_dir = game_dir.unwrap_or_else(|| PathBuf::from("game"));
    let game_dir = game_dir
        .canonicalize()
        .with_context(|| format!("game directory '{}' not found", game_dir.display()))?;
    if !game_dir.join("main.js").is_file() {
        bail!("'{}' has no main.js", game_dir.display());
    }
    info!(
        "Deflorta {} on {}/{}, game {}, {} mode",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        game_dir.display(),
        if test_script.is_some() {
            "headless test"
        } else {
            "windowed"
        },
    );

    let started = Instant::now();
    let mut script = script::ScriptHost::new(game_dir.clone())?;
    script
        .run_main()
        .context("failed to start the game script")?;
    let assets = assets::Assets::new(game_dir.clone());
    let ui = ui::Ui::new(ui::text::TextSystem::new(&game_dir));

    if let Some(test_script) = test_script {
        let engine = engine::Engine::new(script, assets, ui, None);
        info!("Startup took {:.0?}", started.elapsed());
        return headless::run(engine, &test_script);
    }

    let engine = engine::Engine::new(script, assets, ui, audio::Audio::new());
    info!("Startup took {:.0?}", started.elapsed());
    let mut app = app::App::new(engine);
    EventLoop::new()?.run_app(&mut app)?;
    app.take_error().map_or_else(|| Ok(()), Err)
}

fn main() -> ExitCode {
    init_logging();
    let result = run();
    match result {
        Ok(()) => {
            info!("Exited normally");
            ExitCode::SUCCESS
        }
        Err(err) => {
            error!("{err:#}");
            ExitCode::FAILURE
        }
    }
}
