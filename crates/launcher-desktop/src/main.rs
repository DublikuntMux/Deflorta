use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Parser;
use deflorta_assets::GameInspection;
use log::{error, info};

const ARCHIVE: &str = "game.dm";

#[derive(Parser)]
#[command(version, about = "Run a Deflorta game")]
struct Cli {
    /// Game directory or .dm archive (default: game.dm beside this executable).
    path: Option<PathBuf>,
    /// Run a test script headlessly.
    #[arg(long, value_name = "SCRIPT.json", conflicts_with = "inspect")]
    test: Option<PathBuf>,
    /// Evaluate startup without a window and print game information as JSON.
    #[arg(long)]
    inspect: bool,
    /// More log output (-v debug, -vv trace). `RUST_LOG` overrides this.
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

fn game_path(path: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = path {
        return Ok(path);
    }
    let exe = std::env::current_exe().context("cannot locate the launcher executable")?;
    let dir = exe
        .parent()
        .context("the launcher has no parent directory")?;
    Ok(dir.join(ARCHIVE))
}

fn run(cli: Cli) -> Result<()> {
    let path = game_path(cli.path)?;
    let files = deflorta::GameFiles::open(&path)
        .with_context(|| format!("cannot open the game at {}", path.display()))?;
    if cli.inspect {
        let font_families = deflorta::font_families(&files);
        let config = deflorta::boot(files)?;
        let inspection = GameInspection {
            id: config.id,
            title: config.title,
            version: config.version.map(|version| {
                version
                    .as_str()
                    .map_or_else(|| version.to_string(), str::to_owned)
            }),
            font: config.font,
            font_families,
        };
        println!("{}", serde_json::to_string(&inspection)?);
        Ok(())
    } else {
        deflorta::run(files, cli.test.as_deref())
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let filter = match cli.verbose {
        0 => deflorta::DEFAULT_LOG_FILTER,
        1 => "warn,deflorta=debug",
        _ => "warn,deflorta=trace",
    };
    deflorta::init_logging(filter);
    match run(cli) {
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
