//! Player-facing executable of a published game. It runs `game.dm` from its
//! own directory, or the archive or game directory given as the argument.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use log::{error, info};

const ARCHIVE: &str = "game.dm";

fn game_path() -> Result<PathBuf> {
    if let Some(path) = std::env::args_os().nth(1) {
        return Ok(PathBuf::from(path));
    }
    let exe = std::env::current_exe().context("cannot locate the launcher executable")?;
    let dir = exe
        .parent()
        .context("the launcher has no parent directory")?;
    Ok(dir.join(ARCHIVE))
}

fn run() -> Result<()> {
    let path = game_path()?;
    let files = deflorta::GameFiles::open(&path)
        .with_context(|| format!("cannot open the game at {}", path.display()))?;
    deflorta::run(files, None)
}

fn main() -> ExitCode {
    deflorta::init_logging(deflorta::DEFAULT_LOG_FILTER);
    match run() {
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
