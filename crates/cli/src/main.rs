//! `deflorta`: create, check, run, translate, bundle and publish games.

mod api;
mod bundle;
mod check;
mod create;
mod graph;
mod pack;
mod project;
mod report;
mod translate;

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use deflorta::GameFiles;
use oxc::allocator::Allocator;

use crate::pack::{ArchiveFile, Contents};
use crate::project::Project;

#[derive(Parser)]
#[command(name = "deflorta", version, about = "Tools for Deflorta visual novels")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create a new game project.
    Create {
        /// Directory to create.
        path: PathBuf,
        /// Game title (defaults to the directory name).
        #[arg(long)]
        title: Option<String>,
        /// Save-directory id (defaults to a slug of the title).
        #[arg(long)]
        id: Option<String>,
    },
    /// Run a game from its project directory (or a .dm archive) for development.
    Run {
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Play a test script headlessly instead of opening a window.
        #[arg(long, value_name = "SCRIPT.json")]
        test: Option<PathBuf>,
        /// More log output (-v debug, -vv trace). `RUST_LOG` overrides this.
        #[arg(short, long, action = clap::ArgAction::Count)]
        verbose: u8,
    },
    /// Check a game for errors and likely mistakes.
    Check {
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Only analyze the code; do not run the game's scripts.
        #[arg(long)]
        no_boot: bool,
    },
    /// Bundle the scripts and pack the game into a game.dm archive.
    Bundle {
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Archive to write (default: <project>/build/game.dm).
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// LZ4HC compression level, 1–12.
        #[arg(long, default_value_t = pack::DEFAULT_LEVEL, value_parser = clap::value_parser!(i32).range(1..=12))]
        level: i32,
        /// Keep the bundled JavaScript readable.
        #[arg(long)]
        no_minify: bool,
        /// Also write the bundled JavaScript to this file.
        #[arg(long, value_name = "FILE")]
        emit_js: Option<PathBuf>,
    },
    /// Create a folder with the game launcher and its game.dm archive.
    Publish {
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Output folder (default: <project>/dist/<os>-<arch>).
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Launcher executable to ship (default: deflorta-launcher next to this program).
        #[arg(long)]
        launcher: Option<PathBuf>,
        /// Executable name (default: the game's id).
        #[arg(long)]
        name: Option<String>,
        /// LZ4HC compression level, 1–12.
        #[arg(long, default_value_t = pack::DEFAULT_LEVEL, value_parser = clap::value_parser!(i32).range(1..=12))]
        level: i32,
    },
    /// Extract and manage translations in tl/<language>.json.
    Translate {
        #[command(subcommand)]
        command: TranslateCommand,
    },
    /// List the contents of a game.dm archive.
    Info { archive: PathBuf },
    /// Write deflorta.d.ts (and jsconfig.json if missing) for editor support.
    Types {
        #[arg(default_value = ".")]
        path: PathBuf,
    },
}

#[derive(Subcommand)]
enum TranslateCommand {
    /// Add new text to translation files, keeping existing translations.
    Update {
        /// Languages to update (default: every tl/*.json).
        languages: Vec<String>,
        /// Remove entries for text that is no longer in the game.
        #[arg(long)]
        prune: bool,
        #[arg(short, long, default_value = ".")]
        project: PathBuf,
    },
    /// Show translation progress per language.
    Status {
        #[arg(short, long, default_value = ".")]
        project: PathBuf,
    },
    /// List text that is not translated yet.
    Missing {
        language: String,
        #[arg(short, long, default_value = ".")]
        project: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let filter = match &cli.command {
        Command::Run { verbose: 0, .. } => deflorta::DEFAULT_LOG_FILTER,
        Command::Run { verbose: 1, .. } => "warn,deflorta=debug",
        Command::Run { .. } => "warn,deflorta=trace",
        _ => "warn",
    };
    deflorta::init_logging(filter);
    match run(cli.command) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<ExitCode> {
    match command {
        Command::Create { path, title, id } => {
            create::create(&path, title.as_deref(), id.as_deref())?;
            println!("Created {}", path.display());
            println!("  deflorta run {}      play it", path.display());
            println!("  deflorta check {}    find mistakes", path.display());
            println!(
                "  deflorta publish {}  build a folder to share",
                path.display()
            );
        }
        Command::Run { path, test, .. } => {
            deflorta::run(GameFiles::open(&path)?, test.as_deref())?;
        }
        Command::Check { path, no_boot } => {
            let project = Project::open(&path)?;
            let outcome = check::check(&project, !no_boot);
            print!("{}", outcome.report.render(&outcome.sources));
            println!("{}: {}", project.dir.display(), outcome.report.summary());
            if outcome.report.has_errors() {
                return Ok(ExitCode::FAILURE);
            }
        }
        Command::Bundle {
            path,
            output,
            level,
            no_minify,
            emit_js,
        } => {
            let project = Project::open(&path)?;
            let output = output.unwrap_or_else(|| project.build_dir().join("game.dm"));
            build_archive(
                &project,
                &output,
                level,
                !no_minify,
                emit_js.as_deref(),
                &[],
            )?;
        }
        Command::Publish {
            path,
            output,
            launcher,
            name,
            level,
        } => publish(&path, output, launcher, name, level)?,
        Command::Translate { command } => return translate(command),
        Command::Info { archive } => info(&archive)?,
        Command::Types { path } => {
            Project::open(&path)?;
            create::write_types(&path)?;
            println!("Wrote {}", path.join("deflorta.d.ts").display());
        }
    }
    Ok(ExitCode::SUCCESS)
}

#[allow(clippy::cast_precision_loss)]
fn size(bytes: u64) -> String {
    match bytes {
        0..1024 => format!("{bytes} B"),
        1024..1_048_576 => format!("{:.1} KiB", bytes as f64 / 1024.0),
        _ => format!("{:.1} MiB", bytes as f64 / 1_048_576.0),
    }
}

fn build_archive(
    project: &Project,
    output: &Path,
    level: i32,
    minify: bool,
    emit_js: Option<&Path>,
    excluded: &[&Path],
) -> Result<()> {
    let allocator = Allocator::default();
    let graph = project.graph(&allocator);
    if graph.report.has_errors() {
        eprint!("{}", graph.report.render(&project::sources(&graph)));
        bail!("the game's scripts have errors");
    }
    let script = bundle::bundle(&graph, &graph::builtin_exports(), minify)?;
    if let Some(path) = emit_js {
        std::fs::write(path, &script)
            .with_context(|| format!("cannot write {}", path.display()))?;
    }
    let script_size = script.len();
    let data_path = output.with_extension("dm.data");
    let partial_path = output.with_extension("dm.partial");
    let mut excluded = excluded.to_vec();
    excluded.extend([output, data_path.as_path(), partial_path.as_path()]);
    let mut files: Vec<ArchiveFile> = project
        .assets(&excluded)?
        .into_iter()
        .map(|(path, file)| ArchiveFile {
            path,
            contents: Contents::File(file),
        })
        .collect();
    files.push(ArchiveFile {
        path: "main.js".into(),
        contents: Contents::Bytes(script.into_bytes()),
    });
    let stats = pack::write_archive(output, files, level)?;
    let modules = graph.order.len();
    println!(
        "Bundled {modules} module{} into main.js ({})",
        if modules == 1 { "" } else { "s" },
        size(script_size as u64)
    );
    println!(
        "Wrote {}: {} files, {} → {}",
        output.display(),
        stats.files,
        size(stats.size),
        size(stats.stored)
    );
    Ok(())
}

fn publish(
    path: &Path,
    output: Option<PathBuf>,
    launcher: Option<PathBuf>,
    name: Option<String>,
    level: i32,
) -> Result<()> {
    let project = Project::open(path)?;
    if let Some(name) = &name {
        validate_executable_name(name)?;
    }
    let outcome = check::check(&project, false);
    print!("{}", outcome.report.render(&outcome.sources));
    if outcome.report.has_errors() {
        bail!("the game has errors ({})", outcome.report.summary());
    }

    let launcher = match launcher {
        Some(path) => path,
        None => std::env::current_exe()?
            .parent()
            .context("cannot locate this program's directory")?
            .join(format!("deflorta-launcher{}", std::env::consts::EXE_SUFFIX)),
    };
    if !launcher.is_file() {
        bail!(
            "launcher {} not found; build it with `cargo build --release -p deflorta-launcher` or pass --launcher",
            launcher.display()
        );
    }

    let output = output.unwrap_or_else(|| {
        project.dist_dir().join(format!(
            "{}-{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ))
    });
    let output = project::absolute_path(&output)?;
    if project.dir.starts_with(&output) {
        bail!("the publish output must not contain the project directory");
    }
    std::fs::create_dir_all(&output)?;
    let archive = output.join("game.dm");
    build_archive(&project, &archive, level, true, None, &[&output])?;

    // Running the bundle verifies it and tells us the game's id and title.
    let config =
        deflorta::boot(GameFiles::open(&archive)?).context("the bundled game failed to start")?;
    let name = name.unwrap_or_else(|| create::slug(&config.id));
    let executable = output.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    if executable == archive {
        bail!("the launcher name conflicts with game.dm");
    }
    if launcher.canonicalize()?
        == executable
            .canonicalize()
            .unwrap_or_else(|_| executable.clone())
    {
        bail!("the launcher source and destination are the same file");
    }
    std::fs::copy(&launcher, &executable)
        .with_context(|| format!("cannot copy the launcher to {}", executable.display()))?;
    println!("Published '{}' to {}", config.title, output.display());
    println!("  {}  (launcher)", executable.display());
    println!("  {}  (game data)", archive.display());
    Ok(())
}

fn validate_executable_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.chars().any(|c| {
            c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
    {
        bail!("the launcher name must be a single valid file name");
    }
    if name == "game.dm" && std::env::consts::EXE_SUFFIX.is_empty() {
        bail!("the launcher name conflicts with game.dm");
    }
    Ok(())
}

fn translate(command: TranslateCommand) -> Result<ExitCode> {
    let requested = match &command {
        TranslateCommand::Update { languages, .. } => languages.as_slice(),
        TranslateCommand::Missing { language, .. } => std::slice::from_ref(language),
        TranslateCommand::Status { .. } => &[],
    };
    for language in requested {
        if language.is_empty()
            || !language
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            bail!("invalid language id '{language}'");
        }
    }
    let project_path = match &command {
        TranslateCommand::Update { project, .. }
        | TranslateCommand::Status { project }
        | TranslateCommand::Missing { project, .. } => project.clone(),
    };
    let project = Project::open(&project_path)?;
    let extracted = translate::extract(&project)?;
    eprint!("{}", extracted.report.render(&extracted.sources));
    let print_progress = |p: &translate::Progress| {
        let percent = (p.translated * 100).checked_div(p.total).unwrap_or(100);
        let mut line = format!(
            "{}: {}/{} translated ({percent}%)",
            p.language, p.translated, p.total
        );
        if p.added > 0 {
            let _ = write!(line, ", {} new", p.added);
        }
        if p.obsolete > 0 {
            let _ = write!(line, ", {} obsolete (remove with --prune)", p.obsolete);
        }
        println!("{line}");
    };
    match command {
        TranslateCommand::Update {
            languages, prune, ..
        } => {
            let languages = if languages.is_empty() {
                translate::languages(&project)
            } else {
                languages
            };
            if languages.is_empty() {
                bail!(
                    "no tl/*.json files yet; name a language, e.g. `deflorta translate update uk`"
                );
            }
            for language in &languages {
                if language.is_empty()
                    || !language
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                {
                    bail!("invalid language id '{language}'");
                }
                let progress = translate::update(&project, &extracted.strings, language, prune)?;
                print_progress(&progress);
            }
        }
        TranslateCommand::Status { .. } => {
            let languages = translate::languages(&project);
            if languages.is_empty() {
                println!(
                    "No translations yet ({} translatable strings).",
                    extracted.strings.len()
                );
            }
            for language in &languages {
                print_progress(&translate::status(&project, &extracted.strings, language)?);
            }
        }
        TranslateCommand::Missing { language, .. } => {
            let mut out = String::new();
            for text in translate::missing(&project, &extracted.strings, &language)? {
                out.push_str(&serde_json::to_string(&text)?);
                out.push('\n');
            }
            print_listing(&out)?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn info(path: &Path) -> Result<()> {
    let files = GameFiles::archive(path)?;
    let archive = files.as_archive().expect("opened as an archive");
    let (mut total, mut stored) = (0, 0);
    let mut out = format!("{:>10}  {:>10}  path\n", "size", "stored");
    for (entry, entry_size) in archive.entries() {
        let entry_stored = archive.stored_size(entry).unwrap_or(0);
        total += entry_size;
        stored += entry_stored;
        let _ = writeln!(
            out,
            "{:>10}  {:>10}  {entry}",
            size(entry_size),
            size(entry_stored)
        );
    }
    let _ = writeln!(
        out,
        "{} files, {} → {}",
        archive.entries().count(),
        size(total),
        size(stored)
    );
    print_listing(&out)
}

/// Prints command output meant for pipes; a closed pipe (`| head`) is not an error.
fn print_listing(text: &str) -> Result<()> {
    match std::io::stdout().lock().write_all(text.as_bytes()) {
        Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => Ok(()),
        result => Ok(result?),
    }
}
