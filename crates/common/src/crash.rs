//! Panic-safe access to game identity and the persistence queue, independent of the VM.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub type StorageFlush = Arc<dyn Fn(Duration) -> bool + Send + Sync>;

#[derive(Clone, Default)]
pub struct CrashContext {
    pub directory: Option<PathBuf>,
    pub title: Option<String>,
    pub id: Option<String>,
    pub flush: Option<StorageFlush>,
}

static CONTEXT: Mutex<CrashContext> = Mutex::new(CrashContext {
    directory: None,
    title: None,
    id: None,
    flush: None,
});

/// Never waits on a lock held by a panicking thread, nor uses poisoned state.
#[must_use]
pub fn snapshot() -> Option<CrashContext> {
    CONTEXT.try_lock().ok().map(|context| context.clone())
}

pub fn set_game(directory: PathBuf, title: String, id: String) {
    if let Ok(mut context) = CONTEXT.lock() {
        context.directory = Some(directory);
        context.title = Some(title);
        context.id = Some(id);
    }
}

pub fn set_storage(directory: PathBuf, flush: StorageFlush) {
    if let Ok(mut context) = CONTEXT.lock() {
        context.directory = Some(directory);
        context.flush = Some(flush);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panic_context_snapshot_never_waits_for_a_held_lock() {
        let guard = CONTEXT.lock().unwrap();
        assert!(snapshot().is_none());
        drop(guard);
        assert!(snapshot().is_some());
    }
}
