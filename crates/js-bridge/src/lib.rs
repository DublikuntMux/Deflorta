#![recursion_limit = "256"]

pub mod script;
mod storage;
use deflorta_assets::files;
use deflorta_common::worker;
#[cfg(test)]
fn workspace_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
