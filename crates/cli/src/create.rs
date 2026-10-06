//! `deflorta create`: a new game project that runs, checks and publishes as is.

use std::path::Path;

use anyhow::{Context, Result, bail};

const MAIN: &str = include_str!("../templates/main.js");
pub const JSCONFIG: &str = include_str!("../templates/jsconfig.json");
const GITIGNORE: &str = include_str!("../templates/gitignore");

/// Noto Sans, the default font, with its license.
const FONTS: &[(&str, &[u8])] = &[
    (
        "NotoSans-Regular.ttf",
        include_bytes!("../../../game/fonts/NotoSans-Regular.ttf"),
    ),
    (
        "NotoSans-Bold.ttf",
        include_bytes!("../../../game/fonts/NotoSans-Bold.ttf"),
    ),
    (
        "NotoSans-Italic.ttf",
        include_bytes!("../../../game/fonts/NotoSans-Italic.ttf"),
    ),
    (
        "LICENSE-noto.txt",
        include_bytes!("../../../game/fonts/LICENSE-noto.txt"),
    ),
];

/// A save-directory id from a title: lowercase ASCII words joined by dashes.
pub fn slug(title: &str) -> String {
    let mut slug = String::new();
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() {
        "my-game".to_owned()
    } else {
        slug.to_owned()
    }
}

/// Writes the type declarations and editor configuration into a project.
pub fn write_types(dir: &Path) -> Result<()> {
    std::fs::write(dir.join("deflorta.d.ts"), deflorta::TYPE_DECLARATIONS)?;
    let jsconfig = dir.join("jsconfig.json");
    if !jsconfig.exists() {
        std::fs::write(jsconfig, JSCONFIG)?;
    }
    Ok(())
}

pub fn create(dir: &Path, title: Option<&str>, id: Option<&str>) -> Result<()> {
    if dir.exists() && dir.read_dir()?.next().is_some() {
        bail!("{} already exists and is not empty", dir.display());
    }
    let name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .context("the project path has no name")?;
    let title = title.unwrap_or(name);
    let id = id.map_or_else(|| slug(title), str::to_owned);
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("the id may only contain ASCII letters, digits, '-' and '_'");
    }

    for sub in ["fonts", "images", "audio", "movies", "tl"] {
        std::fs::create_dir_all(dir.join(sub))?;
    }
    // Split the template before inserting user text so placeholder-like
    // strings inside the title or id remain literal.
    let (prefix, rest) = MAIN.split_once("__ID__").expect("template id");
    let (middle, suffix) = rest.split_once("__TITLE__").expect("template title");
    let main = format!(
        "{prefix}{}{middle}{}{suffix}",
        serde_json::to_string(&id)?,
        serde_json::to_string(title)?
    );
    std::fs::write(dir.join("main.js"), main)?;
    std::fs::write(dir.join(".gitignore"), GITIGNORE)?;
    for (name, data) in FONTS {
        std::fs::write(dir.join("fonts").join(name), data)?;
    }
    write_types(dir)
}

#[cfg(test)]
mod tests {
    use super::slug;

    #[test]
    fn slugs_are_save_directory_safe() {
        assert_eq!(slug("My Great Game!"), "my-great-game");
        assert_eq!(slug("  Ёлка 2  "), "2");
        assert_eq!(slug("Привіт"), "my-game");
    }
}
