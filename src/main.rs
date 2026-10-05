//! Deflorta — a visual novel engine scripted in JavaScript.

mod app;
mod assets;
mod audio;
mod engine;
mod headless;
mod render;
mod script;
mod ui;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use winit::event_loop::EventLoop;

const USAGE: &str = "usage: deflorta [GAME_DIR] [--test SCRIPT.json]";

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

    let mut script = script::ScriptHost::new(game_dir.clone())?;
    script
        .run_main()
        .context("failed to start the game script")?;
    let assets = assets::Assets::new(game_dir.clone());
    let ui = ui::Ui::new(ui::text::TextSystem::new(&game_dir));

    if let Some(test_script) = test_script {
        let engine = engine::Engine::new(script, assets, ui, None);
        return headless::run(engine, &test_script);
    }

    let engine = engine::Engine::new(script, assets, ui, audio::Audio::new());
    let mut app = app::App::new(engine);
    EventLoop::new()?.run_app(&mut app)?;
    match app.take_error() {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("deflorta: {err:#}");
            ExitCode::FAILURE
        }
    }
}
