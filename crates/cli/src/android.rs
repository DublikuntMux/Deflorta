use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};
use clap::ValueEnum;
use deflorta_data::GameInspection;
use serde_json::json;

use crate::{create, distribution};

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Apk,
    Aab,
}

pub struct Options {
    pub package: Option<String>,
    pub format: Format,
    pub version_code: u32,
}

pub fn abi(platform: &str) -> Result<&'static str> {
    match platform {
        "android-aarch64" => Ok("arm64-v8a"),
        "android-x86_64" => Ok("x86_64"),
        _ => bail!(
            "unsupported Android platform '{platform}'; use android-aarch64 or android-x86_64"
        ),
    }
}

fn validate_package(package: &str) -> Result<()> {
    ensure!(
        package.contains('.')
            && package.split('.').all(|part| {
                part.starts_with(|c: char| c.is_ascii_alphabetic())
                    && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            }),
        "Android package must have at least two dot-separated identifiers starting with a letter"
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn publish(
    output: &Path,
    archive: &Path,
    runtime: &Path,
    config: &GameInspection,
    platform: &str,
    debug: bool,
    name: Option<&str>,
    options: &Options,
) -> Result<()> {
    let abi = abi(platform)?;
    let package = options.package.clone().unwrap_or_else(|| {
        format!(
            "org.deflorta.game_{}",
            create::slug(&config.id).replace('-', "_")
        )
    });
    validate_package(&package)?;
    if !debug {
        for variable in [
            "DEFLORTA_KEYSTORE",
            "DEFLORTA_KEYSTORE_PASSWORD",
            "DEFLORTA_KEY_ALIAS",
            "DEFLORTA_KEY_PASSWORD",
        ] {
            ensure!(
                std::env::var_os(variable).is_some_and(|value| !value.is_empty()),
                "release Android exports need {variable}; use --debug for a debug-signed APK"
            );
        }
    }
    let project = output.join("android");
    let template = distribution::template()?.join("android");
    ensure!(
        template.join("gradlew").is_file(),
        "Android export template not found; rebuild the distribution"
    );
    // Clear it so a second export cannot retain another ABI or stale assets.
    if project.exists() {
        ensure!(
            project.join("deflorta-export.json").is_file(),
            "refusing to replace {}; it is not a generated Android export",
            project.display()
        );
        std::fs::remove_dir_all(&project)?;
    }
    distribution::copy_tree(&template, &project)?;
    let main = project.join("app/src/main");
    distribution::copy_tree(&runtime.join("jniLibs"), &main.join("jniLibs"))?;
    ensure!(
        main.join("jniLibs")
            .join(abi)
            .join("libdeflorta_android.so")
            .is_file(),
        "missing Android native runtime"
    );
    std::fs::create_dir_all(main.join("assets"))?;
    std::fs::copy(archive, main.join("assets/game.dm"))?;
    std::fs::write(
        project.join("deflorta-export.json"),
        serde_json::to_vec_pretty(&json!({
            "applicationId": package,
            "title": config.title,
            "versionCode": options.version_code,
            "versionName": config.version.as_deref().unwrap_or("1.0"),
        }))?,
    )?;
    let variant = if debug { "debug" } else { "release" };
    let task = match (options.format, debug) {
        (Format::Apk, true) => "assembleDebug",
        (Format::Apk, false) => "assembleRelease",
        (Format::Aab, true) => "bundleDebug",
        (Format::Aab, false) => "bundleRelease",
    };
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new("cmd");
        command.args(["/c", "gradlew.bat"]);
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut command = Command::new("sh");
        command.arg("gradlew");
        command
    };
    let status = command
        .current_dir(&project)
        .args(["--console=plain", task])
        .status()
        .context("cannot start the Gradle wrapper (requires Java 17+ and ANDROID_HOME)")?;
    ensure!(status.success(), "Android Gradle export failed ({status})");
    let (source, extension) = match options.format {
        Format::Apk => (format!("apk/{variant}/app-{variant}.apk"), "apk"),
        Format::Aab => (format!("bundle/{variant}/app-{variant}.aab"), "aab"),
    };
    let name = name.map_or_else(|| create::slug(&config.id), str::to_owned);
    let artifact = output.join(format!("{name}.{extension}"));
    std::fs::copy(project.join("app/build/outputs").join(source), &artifact)
        .context("Gradle did not produce the expected Android artifact")?;
    println!(
        "Published '{}' to {} ({package})",
        config.title,
        artifact.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_android_application_ids() {
        for package in ["org.deflorta.game_demo", "com.example.Game1"] {
            validate_package(package).unwrap();
        }
        for package in [
            "",
            "demo",
            "com..demo",
            "com.1demo",
            "com.demo-game",
            "com.demo;exec",
            "com.демо",
        ] {
            assert!(validate_package(package).is_err(), "accepted {package}");
        }
    }
}
