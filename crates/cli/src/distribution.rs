use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use anyhow::{Context, Result, bail};
use deflorta_assets::GameInspection;

pub fn root() -> Result<PathBuf> {
    std::env::current_exe()?
        .parent()
        .context("cannot locate this program's directory")
        .map(Path::to_owned)
}

pub fn template() -> Result<PathBuf> {
    let path = root()?.join("template");
    if !path.is_dir() {
        bail!(
            "template folder {} not found; keep template/ beside deflorta or run python3 scripts/build-dist.py",
            path.display()
        );
    }
    Ok(path)
}

pub fn platform() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

pub fn runtime(platform: &str, debug: bool) -> Result<PathBuf> {
    if platform.is_empty()
        || !platform
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    {
        bail!("the platform must contain only ASCII letters, digits, '-' and '_'");
    }
    let path = root()?
        .join("target")
        .join(platform)
        .join(if debug { "debug" } else { "release" });
    let entry = if platform.starts_with("android-") {
        path.join("jniLibs")
            .join(crate::android::abi(platform)?)
            .join("libdeflorta.so")
    } else {
        launcher(&path)
    };
    if !entry.is_file() {
        let android = if platform.starts_with("android-") {
            " --android"
        } else {
            ""
        };
        bail!(
            "runtime {} not found; run python3 scripts/build-dist.py{android} or install this platform's runtime folder",
            entry.display()
        );
    }
    Ok(path)
}

pub fn launcher(runtime: &Path) -> PathBuf {
    let windows = runtime
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("windows-"));
    runtime.join(if windows {
        "deflorta-launcher.exe"
    } else {
        "deflorta-launcher"
    })
}

pub fn run(path: &Path, test: Option<&Path>, verbose: u8) -> Result<ExitCode> {
    let runtime = runtime(&platform(), true)?;
    let mut command = Command::new(launcher(&runtime));
    // Absolute paths preserve the caller's directory and cannot be mistaken
    // for launcher options when a project or test filename starts with '-'.
    command.arg(std::path::absolute(path)?);
    if let Some(test) = test {
        command.arg("--test").arg(std::path::absolute(test)?);
    }
    if verbose > 0 {
        command.arg(format!("-{}", "v".repeat(usize::from(verbose))));
    }
    let status = command.status().context("cannot start the game launcher")?;
    Ok(status
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .map_or(ExitCode::FAILURE, ExitCode::from))
}

pub fn inspect(path: &Path) -> Result<GameInspection> {
    let runtime = runtime(&platform(), true)?;
    let output = Command::new(launcher(&runtime))
        .arg(path)
        .arg("--inspect")
        .output()
        .context("cannot start the game launcher for inspection")?;
    if !output.status.success() {
        bail!(
            "launcher inspection failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    serde_json::from_slice(&output.stdout)
        .context("the launcher returned invalid startup information")
}

/// Copy all runtime files, preserving nested resources and executable modes.
pub fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    if crate::project::absolute_path(destination)?.starts_with(source.canonicalize()?) {
        bail!(
            "the copy destination must not be inside {}",
            source.display()
        );
    }
    copy_directory(source, destination)
}

fn copy_directory(source: &Path, destination: &Path) -> Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let to = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&entry.path(), &to)?;
        } else {
            std::fs::copy(entry.path(), &to)
                .with_context(|| format!("cannot copy {}", to.display()))?;
        }
    }
    Ok(())
}
