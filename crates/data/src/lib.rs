//! Game data shared by the engine and its tools, without runtime dependencies.

pub mod archive;
pub mod files;
pub mod fonts;
mod jsx;
mod modules;

pub use files::GameFiles;
pub use fonts::font_families;
pub use jsx::compile_jsx;
pub use modules::{BUILTIN_MODULES, is_builtin_module, resolve_specifier};

/// Startup information returned by the launcher's `--inspect` command.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct GameInspection {
    pub id: String,
    pub title: String,
    pub version: Option<String>,
    pub font: String,
    pub font_families: Vec<String>,
}
