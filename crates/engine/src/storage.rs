use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, bail};
use log::{debug, warn};
use serde::Serialize;

use crate::worker::WorkerWake;

#[derive(Clone)]
struct Entry {
    text: Arc<str>,
    modified: u64,
}

#[derive(Serialize)]
pub struct DataEntry {
    name: String,
    modified: u64,
}

enum Job {
    Json {
        name: String,
        revision: u64,
        entry: Option<Entry>,
    },
    Thumbnail {
        path: PathBuf,
        image: Option<Arc<image::RgbaImage>>,
    },
    Flush(Sender<()>),
}

enum Completion {
    Json {
        name: String,
        revision: u64,
        entry: Option<Entry>,
        result: Result<(), String>,
    },
    Thumbnail {
        path: PathBuf,
        available: bool,
        result: Result<(), String>,
    },
}

#[derive(Default)]
pub struct StorageUpdates {
    pub thumbnails: Vec<(PathBuf, bool)>,
    pub errors: Vec<String>,
}

/// A session snapshot with ordered background persistence. Reads see queued
/// writes immediately; failed writes restore the latest persisted value.
pub struct Storage {
    pub dir: PathBuf,
    entries: HashMap<String, Entry>,
    persisted: HashMap<String, Entry>,
    pending: HashMap<String, u64>,
    next_revision: u64,
    jobs: Option<Sender<Job>>,
    results: Receiver<Completion>,
    thread: Option<JoinHandle<()>>,
}

impl Storage {
    pub fn new(dir: PathBuf, wake: WorkerWake) -> Self {
        let (jobs, receiver) = channel();
        let (sender, results) = channel();
        let (initialized, snapshot) = channel();
        let worker_dir = dir.clone();
        let thread = std::thread::Builder::new()
            .name("storage".into())
            .spawn(move || {
                if initialized.send(load_entries(&worker_dir)).is_err() {
                    return;
                }
                while let Ok(job) = receiver.recv() {
                    let completion = match job {
                        Job::Json {
                            name,
                            revision,
                            entry,
                        } => {
                            let path = worker_dir.join(format!("{name}.json"));
                            let result = entry.as_ref().map_or_else(
                                || remove(&path),
                                |entry| {
                                    atomic_write(&path, |tmp| {
                                        Ok(std::fs::write(tmp, entry.text.as_bytes())?)
                                    })
                                },
                            );
                            Completion::Json {
                                name,
                                revision,
                                entry,
                                result: result
                                    .map_err(|e| format!("storage: {}: {e:#}", path.display())),
                            }
                        }
                        Job::Thumbnail { path, image } => {
                            let result = image.map_or_else(
                                || remove(&path),
                                |image| {
                                    atomic_write(&path, |tmp| {
                                        Ok(image.save_with_format(tmp, image::ImageFormat::Png)?)
                                    })
                                },
                            );
                            Completion::Thumbnail {
                                available: path.is_file(),
                                result: result
                                    .map_err(|e| format!("thumbnail: {}: {e:#}", path.display())),
                                path,
                            }
                        }
                        Job::Flush(done) => {
                            let _ = done.send(());
                            continue;
                        }
                    };
                    // Report failures even when shutdown leaves no event loop to
                    // receive them. Normal operation also delivers them to JS.
                    let result = match &completion {
                        Completion::Json { result, .. } | Completion::Thumbnail { result, .. } => {
                            result
                        }
                    };
                    if let Err(error) = result {
                        warn!("{error}");
                    }
                    if sender.send(completion).is_err() {
                        return;
                    }
                    wake.notify();
                }
            })
            .expect("spawn storage worker");
        // Configuration happens before the game boots. The initial disk scan
        // runs on the worker; subsequent reads never touch the filesystem.
        let entries = snapshot.recv().expect("storage snapshot");
        Self {
            dir,
            persisted: entries.clone(),
            entries,
            pending: HashMap::new(),
            next_revision: 0,
            jobs: Some(jobs),
            results,
            thread: Some(thread),
        }
    }

    pub fn read(&self, name: &str) -> Option<String> {
        self.entries.get(name).map(|entry| entry.text.to_string())
    }

    pub fn list(&self) -> Vec<DataEntry> {
        self.entries
            .iter()
            .map(|(name, entry)| DataEntry {
                name: name.clone(),
                modified: entry.modified,
            })
            .collect()
    }

    pub fn write(&mut self, name: String, text: String) -> Result<()> {
        if !valid_name(&name) {
            bail!("storage.write: invalid name '{name}'");
        }
        let entry = Entry {
            text: text.into(),
            modified: timestamp(SystemTime::now()),
        };
        self.queue_json(name, Some(entry))
    }

    pub fn remove(&mut self, name: &str) -> Result<bool> {
        if !valid_name(name) || !self.entries.contains_key(name) {
            return Ok(false);
        }
        self.queue_json(name.into(), None)?;
        Ok(true)
    }

    fn queue_json(&mut self, name: String, entry: Option<Entry>) -> Result<()> {
        let revision = self.next_revision;
        self.next_revision += 1;
        self.send(Job::Json {
            name: name.clone(),
            revision,
            entry: entry.clone(),
        })?;
        replace_entry(&mut self.entries, name.clone(), entry);
        self.pending.insert(name, revision);
        Ok(())
    }

    pub fn thumbnail(&self, path: PathBuf, image: Option<Arc<image::RgbaImage>>) -> Result<()> {
        self.send(Job::Thumbnail { path, image })
    }

    fn send(&self, job: Job) -> Result<()> {
        self.jobs
            .as_ref()
            .expect("storage worker running")
            .send(job)
            .map_err(|_| anyhow::anyhow!("storage worker stopped"))
    }

    /// Only used at shutdown or by tests; interaction never waits for disk.
    pub fn flush(&self) -> Result<()> {
        let (done, receiver) = channel();
        self.send(Job::Flush(done))?;
        receiver
            .recv()
            .map_err(|_| anyhow::anyhow!("storage worker stopped"))
    }

    pub fn poll(&mut self) -> StorageUpdates {
        let mut updates = StorageUpdates::default();
        while let Ok(completion) = self.results.try_recv() {
            match completion {
                Completion::Json {
                    name,
                    revision,
                    entry,
                    result,
                } => {
                    match result {
                        Ok(()) => replace_entry(&mut self.persisted, name.clone(), entry),
                        Err(error) => updates.errors.push(error),
                    }
                    if self.pending.get(&name) == Some(&revision) {
                        let persisted = self.persisted.get(&name).cloned();
                        replace_entry(&mut self.entries, name.clone(), persisted);
                        self.pending.remove(&name);
                    }
                }
                Completion::Thumbnail {
                    path,
                    available,
                    result,
                } => {
                    updates.thumbnails.push((path, available));
                    if let Err(error) = result {
                        updates.errors.push(error);
                    }
                }
            }
        }
        updates
    }
}

impl Drop for Storage {
    fn drop(&mut self) {
        // Closing the queue drains every accepted write, including quit-time
        // autosaves and thumbnails, before the process can exit.
        self.jobs.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
}

fn replace_entry(entries: &mut HashMap<String, Entry>, name: String, entry: Option<Entry>) {
    if let Some(entry) = entry {
        entries.insert(name, entry);
    } else {
        entries.remove(&name);
    }
}

fn timestamp(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

fn load_entries(dir: &Path) -> HashMap<String, Entry> {
    let mut entries = HashMap::new();
    let read = match std::fs::read_dir(dir) {
        Ok(read) => read,
        Err(error) => {
            if error.kind() != std::io::ErrorKind::NotFound {
                warn!("Cannot read storage {}: {error}", dir.display());
            }
            return entries;
        }
    };
    for file in read.flatten() {
        let path = file.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Some(name) = path
            .file_stem()
            .and_then(|s| s.to_str())
            .filter(|name| valid_name(name))
        else {
            continue;
        };
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let modified = file
                    .metadata()
                    .and_then(|m| m.modified())
                    .map_or(0, timestamp);
                entries.insert(
                    name.into(),
                    Entry {
                        text: text.into(),
                        modified,
                    },
                );
            }
            Err(error) => warn!("Cannot read storage {}: {error}", path.display()),
        }
    }
    entries
}

fn atomic_write(path: &Path, write: impl FnOnce(&Path) -> Result<()>) -> Result<()> {
    std::fs::create_dir_all(path.parent().expect("storage parent directory"))?;
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension().unwrap().to_string_lossy()
    ));
    let result = write(&tmp).and_then(|()| Ok(std::fs::rename(&tmp, path)?));
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result?;
    debug!("Saved {}", path.display());
    Ok(())
}

fn remove(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => debug!("Deleted {}", path.display()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "deflorta-storage-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn session_reads_follow_queued_writes_and_deletes_and_restart_restores_disk() {
        let dir = TestDir::new();
        std::fs::write(dir.0.join("prefs.json"), "old").unwrap();
        std::fs::write(dir.0.join("ignored.json.tmp"), "unfinished").unwrap();
        let mut storage = Storage::new(dir.0.clone(), WorkerWake::default());
        assert_eq!(storage.read("prefs").as_deref(), Some("old"));
        assert_eq!(storage.list().len(), 1);
        storage.write("prefs".into(), "new".into()).unwrap();
        assert_eq!(storage.read("prefs").as_deref(), Some("new"));
        assert!(storage.remove("prefs").unwrap());
        assert!(!storage.remove("prefs").unwrap());
        assert!(storage.read("prefs").is_none());
        assert!(storage.list().is_empty());
        storage.write("prefs".into(), "final".into()).unwrap();
        assert!(storage.write("../escape".into(), "bad".into()).is_err());
        assert!(!storage.remove("../escape").unwrap());
        drop(storage);
        assert_eq!(
            std::fs::read_to_string(dir.0.join("prefs.json")).unwrap(),
            "final"
        );
        assert!(!dir.0.join("prefs.json.tmp").exists());
        let storage = Storage::new(dir.0.clone(), WorkerWake::default());
        assert_eq!(storage.read("prefs").as_deref(), Some("final"));
        assert_eq!(storage.list().len(), 1);
        assert!(storage.list()[0].modified > 0);
    }

    #[test]
    fn shutdown_drains_thumbnail_writes_and_deletion_in_order() {
        let dir = TestDir::new();
        let thumbnail = dir.0.join("thumb-1.png");
        let image = Arc::new(image::RgbaImage::from_pixel(
            8,
            4,
            image::Rgba([10, 20, 30, 255]),
        ));
        {
            let storage = Storage::new(dir.0.clone(), WorkerWake::default());
            storage
                .thumbnail(thumbnail.clone(), Some(image.clone()))
                .unwrap();
            storage.thumbnail(thumbnail.clone(), None).unwrap();
            storage
                .thumbnail(thumbnail.clone(), Some(image.clone()))
                .unwrap();
        }
        assert_eq!(image::open(&thumbnail).unwrap().into_rgba8(), *image);
        assert!(!dir.0.join("thumb-1.png.tmp").exists());
        {
            let storage = Storage::new(dir.0.clone(), WorkerWake::default());
            storage.thumbnail(thumbnail.clone(), Some(image)).unwrap();
            storage.thumbnail(thumbnail.clone(), None).unwrap();
        }
        assert!(!thumbnail.exists());
    }

    #[test]
    fn failed_writes_and_removes_roll_back_to_the_persisted_value() {
        let dir = TestDir::new();
        let path = dir.0.join("prefs.json");
        std::fs::write(&path, "old").unwrap();
        let wake = WorkerWake::default();
        let (sender, receiver) = channel();
        wake.set(Arc::new(move || {
            let _ = sender.send(());
        }));
        receiver.try_recv().unwrap();
        let mut storage = Storage::new(dir.0.clone(), wake);
        // A directory in the target's place reliably prevents atomic rename
        // and file deletion on all supported platforms, even as root.
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        storage.write("prefs".into(), "first".into()).unwrap();
        storage.write("prefs".into(), "second".into()).unwrap();
        assert_eq!(storage.read("prefs").as_deref(), Some("second"));
        storage.flush().unwrap();
        receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        let updates = storage.poll();
        assert_eq!(updates.errors.len(), 2);
        assert_eq!(storage.read("prefs").as_deref(), Some("old"));
        assert!(storage.remove("prefs").unwrap());
        storage.flush().unwrap();
        assert_eq!(storage.poll().errors.len(), 1);
        assert_eq!(storage.read("prefs").as_deref(), Some("old"));
        std::fs::remove_dir(&path).unwrap();
        storage.write("prefs".into(), "recovered".into()).unwrap();
        storage.flush().unwrap();
        assert_eq!(storage.poll().errors, Vec::<String>::new());
        assert_eq!(storage.read("prefs").as_deref(), Some("recovered"));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "recovered");
    }
}
