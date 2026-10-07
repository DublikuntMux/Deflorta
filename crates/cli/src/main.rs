//! `deflorta`: create, check, run, translate, bundle and publish games.

mod api;
mod bundle;
mod check;
mod create;
mod distribution;
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
use deflorta_data::GameFiles;
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
        /// Compression level: 1 fast, 2–12 LZ4HC.
        #[arg(long, default_value_t = pack::DEFAULT_LEVEL, value_parser = clap::value_parser!(u8).range(1..=12))]
        level: u8,
        /// Keep the bundled JavaScript readable.
        #[arg(long)]
        no_minify: bool,
        /// Also write the bundled JavaScript to this file.
        #[arg(long, value_name = "FILE")]
        emit_js: Option<PathBuf>,
    },
    /// Create a folder with the target runtime and its game.dm archive.
    Publish {
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Output folder (default: <project>/dist/<os>-<arch>).
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Target platform folder (default: the host's <os>-<arch>).
        #[arg(long)]
        platform: Option<String>,
        /// Ship the debug runtime instead of the release runtime.
        #[arg(long)]
        debug: bool,
        /// Executable name (default: the game's id).
        #[arg(long)]
        name: Option<String>,
        /// Compression level: 1 fast, 2–12 LZ4HC.
        #[arg(long, default_value_t = pack::DEFAULT_LEVEL, value_parser = clap::value_parser!(u8).range(1..=12))]
        level: u8,
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
        Command::Run {
            path,
            test,
            verbose,
        } => {
            return distribution::run(&path, test.as_deref(), verbose);
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
            platform,
            debug,
            name,
            level,
        } => publish(&path, output, platform, debug, name, level)?,
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

fn format_unit(bytes: u64, divisor: u64, unit: &str) -> String {
    let bytes = u128::from(bytes);
    let divisor = u128::from(divisor);
    let tenths = (bytes * 10 + divisor / 2) / divisor;
    format!("{}.{:01} {unit}", tenths / 10, tenths % 10)
}

fn size(bytes: u64) -> String {
    match bytes {
        0..1024 => format!("{bytes} B"),
        1024..1_048_576 => format_unit(bytes, 1024, "KiB"),
        _ => format_unit(bytes, 1_048_576, "MiB"),
    }
}

fn build_archive(
    project: &Project,
    output: &Path,
    level: u8,
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
    let script = bundle::bundle(&graph, &graph::builtin_exports()?, minify)?;
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
    platform: Option<String>,
    debug: bool,
    name: Option<String>,
    level: u8,
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

    let platform = platform.unwrap_or_else(distribution::platform);
    let runtime = distribution::runtime(&platform, debug)?;
    let launcher = distribution::launcher(&runtime);
    let output = output.unwrap_or_else(|| project.dist_dir().join(&platform));
    let output = project::absolute_path(&output)?;
    if project.dir.starts_with(&output) {
        bail!("the publish output must not contain the project directory");
    }
    let installation = distribution::root()?.canonicalize()?;
    if output.starts_with(&installation) || installation.starts_with(&output) {
        bail!("the publish output must not overlap the engine distribution");
    }
    std::fs::create_dir_all(&output)?;
    let archive = output.join("game.dm");
    validate_runtime(&runtime, &archive)?;
    build_archive(&project, &archive, level, true, None, &[&output])?;

    // Running the bundle verifies it and tells us the game's id and title.
    let config = distribution::inspect(&archive).context("the bundled game failed to start")?;
    let name = name.unwrap_or_else(|| create::slug(&config.id));
    let suffix = if platform.starts_with("windows-") {
        ".exe"
    } else {
        ""
    };
    let executable = output.join(format!("{name}{suffix}"));
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
    validate_runtime(&runtime, &executable)?;
    distribution::copy_tree(&runtime, &output)?;
    let copied_launcher = output.join(launcher.file_name().context("launcher has no file name")?);
    if copied_launcher != executable {
        std::fs::rename(&copied_launcher, &executable)
            .with_context(|| format!("cannot rename the launcher to {}", executable.display()))?;
    }
    println!("Published '{}' to {}", config.title, output.display());
    println!("  {}  (launcher)", executable.display());
    println!("  {}  (game data)", archive.display());
    Ok(())
}

fn validate_runtime(runtime: &Path, output: &Path) -> Result<()> {
    let launcher = distribution::launcher(runtime);
    let name = output.file_name().context("output has no file name")?;
    let source = runtime.join(name);
    if source.exists() && source != launcher {
        bail!("{} conflicts with a runtime resource", output.display());
    }
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
                let progress =
                    translate::update(&project, &extracted.translatable, language, prune)?;
                print_progress(&progress);
            }
        }
        TranslateCommand::Status { .. } => {
            let languages = translate::languages(&project);
            if languages.is_empty() {
                println!(
                    "No translations yet ({} translatable strings).",
                    extracted.translatable.len()
                );
            }
            for language in &languages {
                print_progress(&translate::status(
                    &project,
                    &extracted.translatable,
                    language,
                )?);
            }
        }
        TranslateCommand::Missing { language, .. } => {
            let mut out = String::new();
            for text in translate::missing(&project, &extracted.translatable, &language)? {
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
