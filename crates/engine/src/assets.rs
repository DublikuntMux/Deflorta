//! Image sources and background image decoding. Sources are game file paths
//! (see [`GameFiles`]); `user:` paths refer to the per-game data directory
//! (save thumbnails). A `?query` suffix is ignored when resolving, so scripts
//! can bust caches of rewritten files.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use image::ImageReader;
use log::{debug, info, trace, warn};

use crate::files::{GameFiles, normalize_game_path};

/// Background decoding threads.
const DECODE_THREADS: usize = 2;

/// Where an image source lives.
#[derive(Clone)]
pub enum AssetPath {
    /// A normalized path in the game's files.
    Game(String),
    /// A file in the user data directory.
    User(PathBuf),
}

impl AssetPath {
    fn open(&self, files: &GameFiles) -> anyhow::Result<ImageReader<Box<dyn BufReadSeek>>> {
        let reader: Box<dyn BufReadSeek> = match self {
            Self::Game(path) => Box::new(files.open_file(path)?.buffered()),
            Self::User(path) => Box::new(std::io::BufReader::new(std::fs::File::open(path)?)),
        };
        Ok(ImageReader::new(reader).with_guessed_format()?)
    }

    fn decode(&self, files: &GameFiles) -> anyhow::Result<image::RgbaImage> {
        Ok(self.open(files)?.decode()?.into_rgba8())
    }

    fn dimensions(&self, files: &GameFiles) -> anyhow::Result<(u32, u32)> {
        Ok(self.open(files)?.into_dimensions()?)
    }
}

trait BufReadSeek: std::io::BufRead + std::io::Seek {}
impl<T: std::io::BufRead + std::io::Seek> BufReadSeek for T {}

enum ImageState {
    Pending,
    Ready(Arc<image::RgbaImage>),
    /// Handed to the renderer.
    Uploaded,
    Failed,
}

type Job = (String, AssetPath);
type Decoded = (String, Option<image::RgbaImage>);

pub struct Assets {
    files: GameFiles,
    user_dir: Option<PathBuf>,
    sizes: HashMap<String, Option<(u32, u32)>>,
    images: HashMap<String, ImageState>,
    jobs: Sender<Job>,
    results: Receiver<Decoded>,
}

impl Assets {
    pub fn new(files: GameFiles) -> Self {
        let (jobs, job_rx) = channel::<Job>();
        let (result_tx, results) = channel::<Decoded>();
        let job_rx = Arc::new(Mutex::new(job_rx));
        for n in 0..DECODE_THREADS {
            let job_rx = job_rx.clone();
            let result_tx = result_tx.clone();
            let files = files.clone();
            std::thread::Builder::new()
                .name(format!("image decoder {n}"))
                .spawn(move || {
                    loop {
                        let job = job_rx.lock().unwrap().recv();
                        let Ok((src, path)) = job else { return };
                        let started = Instant::now();
                        let image = match path.decode(&files) {
                            Ok(img) => {
                                debug!(
                                    "Decoded '{src}' ({}x{}) in {:.0?}",
                                    img.width(),
                                    img.height(),
                                    started.elapsed()
                                );
                                Some(img)
                            }
                            Err(err) => {
                                warn!("Cannot load image '{src}': {err:#}");
                                None
                            }
                        };
                        if result_tx.send((src, image)).is_err() {
                            return;
                        }
                    }
                })
                .expect("spawn image decoder");
        }
        info!(
            "Assets: {} with {DECODE_THREADS} image decoding threads",
            files.location().display()
        );
        Self {
            files,
            user_dir: None,
            sizes: HashMap::new(),
            images: HashMap::new(),
            jobs,
            results,
        }
    }

    pub fn set_user_dir(&mut self, dir: PathBuf) {
        self.user_dir = Some(dir);
    }

    pub const fn files(&self) -> &GameFiles {
        &self.files
    }

    #[cfg(feature = "dev-console")]
    pub fn loaded_assets(&self) -> Vec<crate::dev_console::diagnostics::LoadedAsset> {
        self.images
            .iter()
            .map(|(source, state)| {
                let size = match state {
                    ImageState::Ready(image) => Some(image.dimensions()),
                    _ => self.sizes.get(source).copied().flatten(),
                };
                crate::dev_console::diagnostics::LoadedAsset {
                    kind: "Image",
                    source: source.clone(),
                    state: match state {
                        ImageState::Pending => "Decoding",
                        ImageState::Ready(_) => "Decoded (CPU)",
                        ImageState::Uploaded => "Uploaded",
                        ImageState::Failed => "Failed",
                    }
                    .into(),
                    detail: size.map_or_else(String::new, |(w, h)| format!("{w}×{h}")),
                    bytes: match state {
                        ImageState::Ready(image) => {
                            Some(image.as_raw().len().try_into().unwrap_or(u64::MAX))
                        }
                        _ => None,
                    },
                }
            })
            .collect()
    }

    pub fn resolve(&self, src: &str) -> Option<AssetPath> {
        let src = src.split('?').next().unwrap_or(src);
        match src.strip_prefix("user:") {
            Some(rest) => {
                let name = normalize_game_path(Path::new(rest))?;
                Some(AssetPath::User(self.user_dir.as_ref()?.join(name)))
            }
            None => normalize_game_path(Path::new(src)).map(AssetPath::Game),
        }
    }

    /// The game file behind a media source (audio, video), logging invalid paths.
    pub fn game_path(&self, src: &str) -> Option<String> {
        if let Some(AssetPath::Game(path)) = self.resolve(src) {
            return Some(path);
        }
        warn!("Media path '{src}' is outside the game directory");
        None
    }

    /// Image dimensions in pixels, read from the file header and cached.
    pub fn image_size(&mut self, src: &str) -> Option<(u32, u32)> {
        if let Some(size) = self.sizes.get(src) {
            return *size;
        }
        let size = self
            .resolve(src)
            .and_then(|path| path.dimensions(&self.files).ok());
        if size.is_none() {
            warn!("Cannot read image '{src}'");
        }
        self.sizes.insert(src.to_owned(), size);
        size
    }

    /// Starts decoding `src` in the background if it is not loaded yet.
    pub fn request(&mut self, src: &str) {
        if self.images.contains_key(src) {
            return;
        }
        let state = if let Some(path) = self.resolve(src) {
            trace!("Queued '{src}' for decoding");
            let _ = self.jobs.send((src.to_owned(), path));
            ImageState::Pending
        } else {
            warn!("Image path '{src}' is outside the game directory");
            ImageState::Failed
        };
        self.images.insert(src.to_owned(), state);
    }

    /// Collects finished decodes.
    pub fn poll(&mut self) {
        while let Ok((src, image)) = self.results.try_recv() {
            let state = image.map_or(ImageState::Failed, |img| ImageState::Ready(Arc::new(img)));
            self.images.insert(src, state);
        }
    }

    /// True when `src` is decoded (or failed), so showing it will not wait.
    pub fn is_settled(&self, src: &str) -> bool {
        !matches!(self.images.get(src), Some(ImageState::Pending) | None)
    }

    pub fn has_pending(&self) -> bool {
        self.images
            .values()
            .any(|s| matches!(s, ImageState::Pending))
    }

    /// Takes decoded pixels for upload; the renderer owns the texture afterwards.
    pub fn take_pixels(&mut self, src: &str) -> Option<Arc<image::RgbaImage>> {
        self.request(src);
        self.poll();
        match self.images.get(src) {
            Some(ImageState::Ready(img)) => {
                let img = img.clone();
                self.images.insert(src.to_owned(), ImageState::Uploaded);
                Some(img)
            }
            _ => None,
        }
    }

    /// Forgets an image whose texture was released (or whose file changed).
    pub fn forget(&mut self, src: &str) {
        debug!("Released '{src}'");
        self.images.remove(src);
        self.sizes.remove(src);
    }
}

#[cfg(all(test, feature = "dev-console"))]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_follow_pixel_ownership_and_asset_release() {
        let files = GameFiles::open(&crate::workspace_dir().join("game")).unwrap();
        let mut assets = Assets::new(files);
        assets.images.insert(
            "probe.png".into(),
            ImageState::Ready(Arc::new(image::RgbaImage::new(8, 4))),
        );
        let decoded = assets.loaded_assets();
        assert_eq!(decoded[0].bytes, Some(128));
        assert_eq!(decoded[0].detail, "8×4");
        assert_eq!(
            assets.take_pixels("probe.png").unwrap().dimensions(),
            (8, 4)
        );
        let uploaded = assets.loaded_assets();
        assert_eq!(uploaded[0].state, "Uploaded");
        assert_eq!(uploaded[0].bytes, None);
        assets.forget("probe.png");
        assert_eq!(assets.loaded_assets().len(), 0);
        assets.request("../outside.png");
        assert_eq!(assets.loaded_assets()[0].state, "Failed");
    }
}
