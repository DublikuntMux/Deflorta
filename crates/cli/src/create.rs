//! `deflorta create`: a new game project that runs, checks and publishes as is.

use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::distribution;

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
    let template = distribution::template()?.join("game");
    std::fs::copy(template.join("deflorta.d.ts"), dir.join("deflorta.d.ts"))
        .context("cannot copy template/game/deflorta.d.ts")?;
    let jsconfig = dir.join("jsconfig.json");
    if !jsconfig.exists() {
        std::fs::copy(template.join("jsconfig.json"), jsconfig)
            .context("cannot copy template/game/jsconfig.json")?;
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

    let template = distribution::template()?.join("game");
    let source = std::fs::read_to_string(template.join("main.js"))
        .context("cannot read template/game/main.js")?;
    // Split the template before inserting user text so placeholder-like
    // strings inside the title or id remain literal.
    let (prefix, rest) = source
        .split_once("__ID__")
        .context("template main.js has no __ID__ placeholder")?;
    let (middle, suffix) = rest
        .split_once("__TITLE__")
        .context("template main.js has no __TITLE__ placeholder")?;
    let main = format!(
        "{prefix}{}{middle}{}{suffix}",
        serde_json::to_string(&id)?,
        serde_json::to_string(title)?
    );
    distribution::copy_tree(&template, dir)?;
    std::fs::write(dir.join("main.js"), main)?;
    Ok(())
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
