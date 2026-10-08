#![recursion_limit = "256"]

#[cfg(feature = "dev-console")]
pub mod dev_console;
pub mod engine;
pub mod render;
pub mod self_voicing;
#[cfg(test)]
use deflorta_assets::GameFiles;
use deflorta_assets::{assets, audio};
use deflorta_common::{data_dir, util, worker};
use deflorta_js_bridge::script;
use deflorta_ui::ui;
#[cfg(test)]
fn workspace_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[cfg(all(test, feature = "dev-console"))]
mod script_tests;
