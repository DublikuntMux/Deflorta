// Clippy's Send analysis of wgpu initialization futures exceeds the default depth.
#![recursion_limit = "256"]

pub mod engine;
pub mod render;
pub mod self_voicing;

#[cfg(feature = "dev-console")]
pub mod dev_console;

use deflorta_assets::{assets, audio};
use deflorta_common::{data_dir, util, worker};
use deflorta_js_bridge::script;
use deflorta_ui::ui;

#[cfg(test)]
use deflorta_assets::GameFiles;

#[cfg(test)]
fn workspace_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[cfg(all(test, feature = "dev-console"))]
mod script_tests;

#[cfg(all(test, feature = "dev-console"))]
fn compiled_fixture(name: &str) -> (tempfile::TempDir, GameFiles) {
    let root = workspace_dir().join("tests").join(name);
    let scripts = tempfile::tempdir().unwrap();
    let source = std::fs::read_to_string(root.join("main.js")).unwrap();
    let compiled = deflorta_script_build::compile_jsx("main.js", &source).unwrap();
    std::fs::write(scripts.path().join("main.js"), compiled.as_bytes()).unwrap();
    let files = GameFiles::with_scripts(&root, scripts.path()).unwrap();
    (scripts, files)
}
