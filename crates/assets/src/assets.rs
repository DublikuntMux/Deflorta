use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use image::ImageReader;
use log::{debug, info, trace, warn};

use crate::files::{GameFiles, normalize_game_path};
use crate::worker::{WakeCallback, WorkerWake};

const DECODE_THREADS: usize = 2;

/// Unreferenced image data, metadata and textures expire together.
pub const ASSET_IDLE_LIMIT: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub enum AssetPath {
    Game(String),
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
    Pending(u64),
    Ready(Arc<image::RgbaImage>),
    /// Handed to the renderer.
    Uploaded,
    Failed,
}

type Job = (String, u64, AssetPath);
type Decoded = (String, u64, Option<image::RgbaImage>);

pub struct Assets {
    files: GameFiles,
    user_dir: Option<PathBuf>,
    sizes: HashMap<String, Option<(u32, u32)>>,
    images: HashMap<String, ImageState>,
    last_used: HashMap<String, Instant>,
    next_job: u64,
    jobs: Sender<Job>,
    results: Receiver<Decoded>,
    wake: WorkerWake,
    pending_writes: HashMap<PathBuf, usize>,
}

impl Assets {
    #[must_use]
    ///
    /// # Panics
    ///
    /// Panics if an image decoder worker thread cannot be started.
    pub fn new(files: GameFiles) -> Self {
        let (jobs, job_rx) = channel::<Job>();
        let (result_tx, results) = channel::<Decoded>();
        let job_rx = Arc::new(Mutex::new(job_rx));
        let wake = WorkerWake::default();
        for n in 0..DECODE_THREADS {
            let job_rx = job_rx.clone();
            let result_tx = result_tx.clone();
            let files = files.clone();
            let wake = wake.clone();
            std::thread::Builder::new()
                .name(format!("image decoder {n}"))
                .spawn(move || {
                    loop {
                        let job = job_rx.lock().unwrap().recv();
                        let Ok((src, generation, path)) = job else {
                            return;
                        };
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
                        if result_tx.send((src, generation, image)).is_err() {
                            return;
                        }
                        wake.notify();
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
            last_used: HashMap::new(),
            next_job: 0,
            jobs,
            results,
            wake,
            pending_writes: HashMap::new(),
        }
    }

    pub fn set_waker(&self, callback: WakeCallback) {
        self.wake.set(callback);
    }

    pub fn set_user_dir(&mut self, dir: PathBuf) {
        self.user_dir = Some(dir);
    }

    #[must_use]
    pub const fn files(&self) -> &GameFiles {
        &self.files
    }

    #[cfg(feature = "dev-console")]
    #[must_use]
    pub fn loaded_assets(&self) -> Vec<deflorta_common::diagnostics::LoadedAsset> {
        self.images
            .iter()
            .map(|(source, state)| {
                let size = match state {
                    ImageState::Ready(image) => Some(image.dimensions()),
                    _ => self.sizes.get(source).copied().flatten(),
                };
                deflorta_common::diagnostics::LoadedAsset {
                    kind: "Image",
                    source: source.clone(),
                    state: match state {
                        ImageState::Pending(_) => "Decoding",
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
    #[must_use]
    pub fn game_path(&self, src: &str) -> Option<String> {
        if let Some(AssetPath::Game(path)) = self.resolve(src) {
            return Some(path);
        }
        warn!("Media path '{src}' is outside the game directory");
        None
    }

    pub fn image_size(&mut self, src: &str) -> Option<(u32, u32)> {
        self.touch(src);
        if let Some(size) = self.sizes.get(src) {
            return *size;
        }
        if matches!(self.resolve(src), Some(AssetPath::User(path)) if self.pending_writes.contains_key(&path))
        {
            return None;
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

    pub fn request(&mut self, src: &str) {
        self.touch(src);
        if self.images.contains_key(src) {
            return;
        }
        let state = if let Some(path) = self.resolve(src) {
            trace!("Queued '{src}' for decoding");
            let generation = self.next_job;
            self.next_job += 1;
            let writing =
                matches!(&path, AssetPath::User(path) if self.pending_writes.contains_key(path));
            if writing || self.jobs.send((src.to_owned(), generation, path)).is_ok() {
                ImageState::Pending(generation)
            } else {
                ImageState::Failed
            }
        } else {
            warn!("Image path '{src}' is outside the game directory");
            ImageState::Failed
        };
        self.images.insert(src.to_owned(), state);
    }

    fn touch(&mut self, src: &str) {
        let now = Instant::now();
        if let Some(used) = self.last_used.get_mut(src) {
            *used = now;
        } else {
            self.last_used.insert(src.to_owned(), now);
        }
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok((src, generation, image)) = self.results.try_recv() {
            // An unload or a new request invalidates results from older jobs.
            if !matches!(self.images.get(&src), Some(ImageState::Pending(job)) if *job == generation)
            {
                continue;
            }
            let state = image.map_or(ImageState::Failed, |img| {
                self.sizes.insert(src.clone(), Some(img.dimensions()));
                ImageState::Ready(Arc::new(img))
            });
            self.images.insert(src, state);
            changed = true;
        }
        changed
    }

    /// True when `src` is decoded (or failed), so showing it will not wait.
    #[must_use]
    pub fn is_settled(&self, src: &str) -> bool {
        !matches!(self.images.get(src), Some(ImageState::Pending(_)) | None)
    }

    #[must_use]
    pub fn has_pending(&self) -> bool {
        self.images
            .values()
            .any(|s| matches!(s, ImageState::Pending(_)))
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

    pub fn forget(&mut self, src: &str) -> bool {
        let image = self.images.remove(src).is_some();
        let size = self.sizes.remove(src).is_some();
        self.last_used.remove(src);
        if image || size {
            debug!("Released '{src}'");
        }
        image || size
    }

    /// A thumbnail may be requested before its queued write reaches disk.
    /// Invalidate old decodes and retry all aliases once persistence completes.
    pub fn user_file_pending(&mut self, path: PathBuf) {
        *self.pending_writes.entry(path).or_default() += 1;
    }

    pub fn user_file_changed(&mut self, path: &Path, available: bool) -> Vec<String> {
        if let Some(count) = self.pending_writes.get_mut(path)
            && *count > 1
        {
            *count -= 1;
            return Vec::new();
        }
        self.pending_writes.remove(path);
        let sources: Vec<_> = self
            .last_used
            .keys()
            .filter(|src| matches!(self.resolve(src), Some(AssetPath::User(file)) if file == path))
            .cloned()
            .collect();
        for source in &sources {
            let requested = self.images.contains_key(source);
            self.forget(source);
            if requested && available {
                self.request(source);
            } else if requested {
                self.images.insert(source.clone(), ImageState::Failed);
                self.sizes.insert(source.clone(), None);
                self.touch(source);
            }
        }
        sources
    }

    pub fn collect_unused(&mut self, retained: &HashSet<String>, now: Instant) -> Vec<String> {
        let mut expired = Vec::new();
        self.last_used.retain(|src, used| {
            if retained.contains(src) {
                *used = now;
                return true;
            }
            if now.saturating_duration_since(*used) < ASSET_IDLE_LIMIT {
                return true;
            }
            self.images.remove(src);
            self.sizes.remove(src);
            debug!("Released idle image '{src}'");
            expired.push(src.clone());
            false
        });
        expired
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn controlled_assets() -> (Assets, Receiver<Job>, Sender<Decoded>) {
        let files = GameFiles::open(&crate::workspace_dir().join("game")).unwrap();
        let mut assets = Assets::new(files);
        let (jobs, requests) = channel();
        let (results, decoded) = channel();
        assets.jobs = jobs;
        assets.results = decoded;
        (assets, requests, results)
    }

    #[test]
    fn idle_cleanup_releases_preloads_pending_failed_uploaded_and_metadata() {
        let (mut assets, requests, results) = controlled_assets();
        for source in ["ready.png", "pending.png", "uploaded.png", "failed.png"] {
            assets.request(source);
            let (source, generation, _) = requests.recv().unwrap();
            match source.as_str() {
                "pending.png" => {}
                "failed.png" => results.send((source, generation, None)).unwrap(),
                _ => results
                    .send((source, generation, Some(image::RgbaImage::new(8, 4))))
                    .unwrap(),
            }
        }
        assert!(assets.poll());
        assert!(assets.take_pixels("uploaded.png").is_some());
        assert_eq!(assets.image_size("../size-only.png"), None);
        let now = Instant::now();
        assets.last_used.values_mut().for_each(|used| *used = now);
        let retained = HashSet::from(["ready.png".to_owned()]);
        assert_eq!(
            assets.collect_unused(
                &retained,
                now + ASSET_IDLE_LIMIT.saturating_sub(Duration::from_secs(1))
            ),
            Vec::<String>::new()
        );
        let expired = assets.collect_unused(&retained, now + ASSET_IDLE_LIMIT);
        assert_eq!(expired.len(), 4);
        assert_eq!(assets.images.len(), 1);
        assert!(matches!(
            assets.images.get("ready.png"),
            Some(ImageState::Ready(_))
        ));
        assert_eq!(assets.sizes.len(), 1);
        assert_eq!(assets.sizes.get("ready.png"), Some(&Some((8, 4))));
        assert_eq!(assets.last_used.len(), 1);
        assert_eq!(
            assets.collect_unused(
                &HashSet::new(),
                now + (ASSET_IDLE_LIMIT * 2).saturating_sub(Duration::from_secs(1))
            ),
            Vec::<String>::new()
        );
        assert_eq!(
            assets.collect_unused(&HashSet::new(), now + ASSET_IDLE_LIMIT * 2),
            ["ready.png"]
        );
    }

    #[test]
    fn unload_discards_late_decodes_and_reload_uses_only_the_new_job() {
        let (mut assets, requests, results) = controlled_assets();
        let source = "images/title screen.png?2";
        assets.request(source);
        let (_, old, _) = requests.recv().unwrap();
        assets.sizes.insert(source.into(), Some((8, 4)));
        assert!(assets.forget(source));
        results
            .send((source.into(), old, Some(image::RgbaImage::new(8, 4))))
            .unwrap();
        assert!(!assets.poll());
        assert!(assets.images.is_empty());
        assert!(assets.sizes.is_empty());
        assets.request(source);
        let (_, new, _) = requests.recv().unwrap();
        assert_ne!(old, new);
        results.send((source.into(), old, None)).unwrap();
        assert!(!assets.poll());
        assert!(!assets.is_settled(source));
        results
            .send((source.into(), new, Some(image::RgbaImage::new(16, 8))))
            .unwrap();
        assert!(assets.poll());
        assert_eq!(assets.take_pixels(source).unwrap().dimensions(), (16, 8));
    }

    #[test]
    fn decoders_publish_success_and_failure_before_waking() {
        let files = GameFiles::open(&crate::workspace_dir().join("game")).unwrap();
        let mut assets = Assets::new(files);
        let (sender, receiver) = channel();
        assets.set_waker(Arc::new(move || {
            let _ = sender.send(());
        }));
        receiver.try_recv().unwrap();
        for (source, success) in [("images/bg room.png", true), ("missing.png", false)] {
            assets.request(source);
            receiver.recv_timeout(Duration::from_secs(5)).unwrap();
            assert!(assets.poll());
            assert!(assets.is_settled(source));
            assert_eq!(assets.take_pixels(source).is_some(), success);
        }
    }

    #[test]
    fn thumbnail_completion_retries_failed_aliases_and_discards_old_decodes() {
        let (mut assets, requests, results) = controlled_assets();
        let dir = PathBuf::from("/saved-data");
        assets.set_user_dir(dir.clone());
        let failed = "user:thumb-1.png?1";
        let pending = "user:thumb-1.png?2";
        assets.request(failed);
        let (_, first, _) = requests.recv().unwrap();
        results.send((failed.into(), first, None)).unwrap();
        assets.poll();
        assert_eq!(assets.image_size(failed), None);
        assets.request(pending);
        let (_, old, _) = requests.recv().unwrap();
        assert_eq!(
            assets
                .user_file_changed(&dir.join("thumb-1.png"), true)
                .len(),
            2
        );
        let mut jobs = HashMap::new();
        for _ in 0..2 {
            let (source, revision, _) = requests.recv().unwrap();
            jobs.insert(source, revision);
        }
        results
            .send((pending.into(), old, Some(image::RgbaImage::new(1, 1))))
            .unwrap();
        assert!(!assets.poll());
        for (source, revision) in jobs {
            results
                .send((source, revision, Some(image::RgbaImage::new(16, 8))))
                .unwrap();
        }
        assert!(assets.poll());
        for source in [failed, pending] {
            assert_eq!(assets.image_size(source), Some((16, 8)));
            assert_eq!(assets.take_pixels(source).unwrap().dimensions(), (16, 8));
        }
    }

    #[test]
    fn thumbnail_decode_waits_for_all_writes_and_a_failed_write_releases_the_wait() {
        let (mut assets, requests, results) = controlled_assets();
        let dir = PathBuf::from("/saved-data");
        assets.set_user_dir(dir.clone());
        let path = dir.join("thumb-1.png");
        let source = "user:thumb-1.png?new";
        assets.user_file_pending(path.clone());
        assets.user_file_pending(path.clone());
        assets.request(source);
        assert!(assets.has_pending());
        assert_eq!(assets.image_size(source), None);
        assert!(requests.try_recv().is_err());
        assert_eq!(assets.user_file_changed(&path, true), Vec::<String>::new());
        assert!(requests.try_recv().is_err());
        assert_eq!(assets.user_file_changed(&path, true), [source]);
        let (_, generation, _) = requests.try_recv().unwrap();
        results
            .send((
                source.into(),
                generation,
                Some(image::RgbaImage::new(16, 8)),
            ))
            .unwrap();
        assert!(assets.poll());
        assert!(assets.is_settled(source));
        assets.user_file_pending(path.clone());
        assets.request("user:thumb-1.png?failed");
        assets.user_file_changed(&path, false);
        assert!(!assets.has_pending());
        assert!(assets.is_settled("user:thumb-1.png?failed"));
        assert!(requests.try_recv().is_err());
    }

    #[cfg(feature = "dev-console")]
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
