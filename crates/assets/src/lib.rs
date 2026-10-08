#![recursion_limit = "256"]

pub mod archive;
pub mod files;
pub mod fonts;
mod jsx;
mod modules;

pub use files::GameFiles;
pub use fonts::font_families;
pub use jsx::compile_jsx;
pub use modules::{BUILTIN_MODULES, is_builtin_module, resolve_specifier};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct GameInspection {
    pub id: String,
    pub title: String,
    pub version: Option<String>,
    pub font: String,
    pub font_families: Vec<String>,
}

pub mod assets;
pub mod audio;
mod media;
pub mod video;
use deflorta_common::worker;
#[cfg(test)]
fn workspace_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
