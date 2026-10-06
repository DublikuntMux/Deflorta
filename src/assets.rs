//! Game file access and background image decoding. Paths are relative to the
//! game directory and may not escape it; `user:` paths refer to the per-game
//! data directory (save thumbnails). A `?query` suffix is ignored when
//! resolving, so scripts can bust caches of rewritten files.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use log::{debug, info, trace, warn};

/// Background decoding threads.
const DECODE_THREADS: usize = 2;

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

enum ImageState {
    Pending,
    Ready(Arc<image::RgbaImage>),
    /// Handed to the renderer.
    Uploaded,
    Failed,
}

type Job = (String, PathBuf);
type Decoded = (String, Option<image::RgbaImage>);

pub struct Assets {
    root: PathBuf,
    user_dir: Option<PathBuf>,
    sizes: HashMap<String, Option<(u32, u32)>>,
    images: HashMap<String, ImageState>,
    jobs: Sender<Job>,
    results: Receiver<Decoded>,
}

impl Assets {
    pub fn new(root: PathBuf) -> Self {
        let (jobs, job_rx) = channel::<Job>();
        let (result_tx, results) = channel::<Decoded>();
        let job_rx = Arc::new(Mutex::new(job_rx));
        for n in 0..DECODE_THREADS {
            let job_rx = job_rx.clone();
            let result_tx = result_tx.clone();
            std::thread::Builder::new()
                .name(format!("image decoder {n}"))
                .spawn(move || {
                    loop {
                        let job = job_rx.lock().unwrap().recv();
                        let Ok((src, path)) = job else { return };
                        let started = Instant::now();
                        let image = match image::open(&path) {
                            Ok(img) => {
                                let img = img.into_rgba8();
                                debug!(
                                    "Decoded '{src}' ({}x{}) in {:.0?}",
                                    img.width(),
                                    img.height(),
                                    started.elapsed()
                                );
                                Some(img)
                            }
                            Err(err) => {
                                warn!("Cannot load image '{src}': {err}");
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
            root.display()
        );
        Self {
            root,
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

    pub fn resolve(&self, src: &str) -> Option<PathBuf> {
        let src = src.split('?').next().unwrap_or(src);
        match src.strip_prefix("user:") {
            Some(rest) => {
                let name = normalize_game_path(Path::new(rest))?;
                Some(self.user_dir.as_ref()?.join(name))
            }
            None => normalize_game_path(Path::new(src)).map(|p| self.root.join(p)),
        }
    }

    /// Image dimensions in pixels, read from the file header and cached.
    pub fn image_size(&mut self, src: &str) -> Option<(u32, u32)> {
        if let Some(size) = self.sizes.get(src) {
            return *size;
        }
        let size = self
            .resolve(src)
            .and_then(|path| image::image_dimensions(&path).ok());
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
