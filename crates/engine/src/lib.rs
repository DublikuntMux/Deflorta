mod app;

#[cfg(not(target_os = "android"))]
mod discord;

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

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use log::{error, info};
use winit::event_loop::EventLoop;

pub use deflorta_assets::{GameFiles, archive, files, font_families};
pub use script::{BUILTIN_MODULES, GameConfig, is_builtin_module, resolve_specifier};

/// Log filter used when `RUST_LOG` is not set: engine and script messages at
/// info, third-party crates only when something goes wrong.
pub const DEFAULT_LOG_FILTER: &str = "warn,deflorta=info";

pub fn init_logging(default_filter: &str) {
    install_panic_hook();
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
    install_panic_hook();
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
    install_panic_hook();
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

fn install_panic_hook() {
    static INSTALLED: std::sync::Once = std::sync::Once::new();
    INSTALLED.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic| {
            let context = deflorta_common::crash::snapshot().unwrap_or_default();
            let backtrace = std::backtrace::Backtrace::force_capture();
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            let thread = std::thread::current();
            let message = panic.payload().downcast_ref::<&str>().copied().or_else(|| {
                panic.payload().downcast_ref::<String>().map(String::as_str)
            }).unwrap_or("non-string panic payload");
            let location = panic.location().map_or_else(
                || "unknown".into(),
                |location| format!("{}:{}:{}", location.file(), location.line(), location.column()),
            );
            let report = format!(
                "Timestamp (Unix seconds): {}\nEngine version: {}\nGame title: {}\nGame id: {}\nThread: {}\nMessage: {message}\nLocation: {location}\nBacktrace:\n{backtrace}\n",
                timestamp.as_secs_f64(),
                env!("CARGO_PKG_VERSION"),
                context.title.as_deref().unwrap_or("unknown"),
                context.id.as_deref().unwrap_or("unknown"),
                thread.name().unwrap_or("unnamed"),
            );
            error!("{report}");
            let name = format!("crash-{}-{}.txt", timestamp.as_nanos(), std::process::id());
            match write_crash_report(context.directory.as_deref(), &name, &report) {
                Ok(path) => error!("Crash report: {}", path.display()),
                Err(error) => error!("Cannot write crash report: {error}"),
            }
            if let Some(flush) = context.flush {
                let completed = flush(Duration::from_secs(2));
                error!("Crash storage flush completed: {completed}");
            }
            previous(panic);
        }));
    });
}

fn write_crash_report(
    directory: Option<&Path>,
    name: &str,
    report: &str,
) -> std::io::Result<PathBuf> {
    use std::io::Write;

    let write = |directory: &Path| {
        std::fs::create_dir_all(directory)?;
        let path = directory.join(name);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(report.as_bytes())?;
        Ok(path)
    };
    if let Some(directory) = directory
        && let Ok(path) = write(&directory.join("crashes"))
    {
        return Ok(path);
    }
    write(&std::env::temp_dir().join("deflorta-crashes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panic_hook_writes_identity_location_backtrace_and_chains_previous_hook() {
        const CHILD: &str = "DEFLORTA_PANIC_REPORT_CHILD";
        if let Some(directory) = std::env::var_os(CHILD) {
            deflorta_common::crash::set_game(
                directory.into(),
                "Crash test".into(),
                "crash-test".into(),
            );
            install_panic_hook();
            panic!("crash report test marker");
        }
        let directory =
            std::env::temp_dir().join(format!("deflorta-crash-test-{}", std::process::id()));
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::panic_hook_writes_identity_location_backtrace_and_chains_previous_hook",
                "--nocapture",
            ])
            .env(CHILD, &directory)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("crash report test marker"));
        let reports: Vec<_> = std::fs::read_dir(directory.join("crashes"))
            .unwrap()
            .collect();
        assert_eq!(reports.len(), 1);
        let report = std::fs::read_to_string(reports[0].as_ref().unwrap().path()).unwrap();
        for expected in [
            "Timestamp",
            "Engine version",
            "Game title: Crash test",
            "Game id: crash-test",
            "Thread:",
            "Message: crash report test marker",
            "Location:",
            "lib.rs:",
            "Backtrace:",
        ] {
            assert!(report.contains(expected), "missing {expected}: {report}");
        }
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn crash_report_falls_back_to_temporary_directory() {
        let name = format!(
            "fallback-test-{}-{}.txt",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let path = write_crash_report(
            Some(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("Cargo.toml")
                    .as_path(),
            ),
            &name,
            "report",
        )
        .unwrap();
        assert_eq!(
            path.parent(),
            Some(std::env::temp_dir().join("deflorta-crashes").as_path())
        );
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "report");
        std::fs::remove_file(path).unwrap();
    }
}
