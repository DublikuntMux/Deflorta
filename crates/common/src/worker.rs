use std::sync::{Arc, Mutex};

pub type WakeCallback = Arc<dyn Fn() + Send + Sync>;

/// Workers publish their result before waking the event thread. Attaching a
/// callback also wakes it, covering results completed before the window exists.
#[derive(Clone, Default)]
pub struct WorkerWake(Arc<Mutex<Option<WakeCallback>>>);

impl WorkerWake {
    pub fn set(&self, callback: WakeCallback) {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(callback);
        self.notify();
    }

    pub fn notify(&self) {
        let callback = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if let Some(callback) = callback {
            callback();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    #[test]
    fn attaching_and_worker_completion_both_wake_the_event_thread() {
        let wake = WorkerWake::default();
        wake.notify();
        let (sender, receiver) = channel();
        wake.set(Arc::new(move || sender.send(()).unwrap()));
        receiver.try_recv().unwrap();
        let worker = wake.clone();
        std::thread::spawn(move || worker.notify()).join().unwrap();
        receiver.try_recv().unwrap();
        wake.notify();
        receiver.try_recv().unwrap();
    }
}
