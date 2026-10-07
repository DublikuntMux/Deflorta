use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};

use crate::files::normalize_game_path;

/// Built-in module specifiers and the names of their runtime source files.
pub const BUILTIN_MODULES: &[(&str, &str)] = &[
    ("deflorta", "deflorta.js"),
    ("deflorta/core", "core.js"),
    ("deflorta/ui", "ui.js"),
    ("deflorta/text", "text.js"),
    ("deflorta/scene", "scene.js"),
    ("deflorta/story", "story.js"),
    ("deflorta/screens", "screens.js"),
];

/// True for the engine's built-in modules (`deflorta`, `deflorta/ui`, …).
pub fn is_builtin_module(specifier: &str) -> bool {
    BUILTIN_MODULES.iter().any(|(name, _)| *name == specifier)
}

/// Resolves an import specifier to a builtin name or a path in the game.
/// Specifiers name files exactly; no extensions are added.
pub fn resolve_specifier(referrer: &str, specifier: &str) -> Result<String> {
    if is_builtin_module(specifier) {
        return Ok(specifier.to_owned());
    }
    let base = if let Some(rest) = specifier.strip_prefix('/') {
        PathBuf::from(rest)
    } else if specifier.starts_with("./") || specifier.starts_with("../") {
        if is_builtin_module(referrer) {
            bail!("builtin module '{referrer}' cannot import '{specifier}'");
        }
        Path::new(referrer)
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(specifier)
    } else {
        bail!("unknown module '{specifier}' (game modules must start with './', '../' or '/')");
    };
    normalize_game_path(&base)
        .ok_or_else(|| anyhow!("module path '{specifier}' escapes the game directory"))
}
