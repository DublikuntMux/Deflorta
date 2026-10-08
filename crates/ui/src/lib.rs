pub mod ui;

use deflorta_assets::files;
use deflorta_assets::{assets, video};
use deflorta_common::{speech as self_voicing, util};

#[cfg(test)]
use deflorta_assets::GameFiles;

#[cfg(test)]
fn workspace_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
