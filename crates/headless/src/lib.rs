#![recursion_limit = "256"]

pub mod headless;
#[cfg(all(test, target_os = "linux"))]
use deflorta_assets::GameFiles;
#[cfg(all(test, target_os = "linux"))]
use deflorta_assets::assets;
use deflorta_engine_core::{engine, render};
#[cfg(all(test, target_os = "linux"))]
use deflorta_js_bridge::script;
#[cfg(all(test, target_os = "linux"))]
use deflorta_ui::ui;
#[cfg(all(test, target_os = "linux"))]
fn workspace_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
