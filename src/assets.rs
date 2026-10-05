//! Game file access. All paths are relative to the game directory and may
//! not escape it.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

/// Normalizes a relative path, refusing anything that leaves the game root.
pub fn normalize_game_path(path: &Path) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str()?),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

pub struct Assets {
    root: PathBuf,
    sizes: HashMap<String, Option<(u32, u32)>>,
}

impl Assets {
    pub fn new(root: PathBuf) -> Self {
        Assets {
            root,
            sizes: HashMap::new(),
        }
    }

    pub fn resolve(&self, src: &str) -> Option<PathBuf> {
        normalize_game_path(Path::new(src)).map(|p| self.root.join(p))
    }

    /// Image dimensions in pixels, read from the file header and cached.
    pub fn image_size(&mut self, src: &str) -> Option<(u32, u32)> {
        if let Some(size) = self.sizes.get(src) {
            return *size;
        }
        let size = self
            .resolve(src)
            .and_then(|path| match image::image_dimensions(&path) {
                Ok(size) => Some(size),
                Err(err) => {
                    eprintln!("[deflorta] cannot read image '{src}': {err}");
                    None
                }
            });
        self.sizes.insert(src.to_owned(), size);
        size
    }

    /// Decodes an image to RGBA8.
    pub fn load_image(&self, src: &str) -> Option<image::RgbaImage> {
        let path = self.resolve(src)?;
        match image::open(&path) {
            Ok(img) => Some(img.into_rgba8()),
            Err(err) => {
                eprintln!("[deflorta] cannot load image '{src}': {err}");
                None
            }
        }
    }
}
