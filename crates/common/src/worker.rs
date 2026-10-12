use std::sync::{Arc, Mutex, OnceLock};

/// Shared executor for background engine services.
///
/// Blocking codecs and native APIs run with `spawn_blocking`; async tasks
/// coordinate queues, I/O and timers.
/// The runtime lives for the process lifetime, including headless launchers.
///
/// # Panics
///
/// Panics if Tokio cannot create its worker threads.
#[must_use]
pub fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("deflorta-worker")
            .enable_all()
            .build()
            .expect("start engine background runtime")
    })
}

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
        runtime()
            .block_on(runtime().spawn_blocking(move || worker.notify()))
            .unwrap();
        receiver.try_recv().unwrap();
        wake.notify();
        receiver.try_recv().unwrap();
    }

    #[test]
    fn blocking_jobs_do_not_stall_async_wakeups() {
        assert!(std::ptr::eq(runtime(), runtime()));
        let (entered, waiting) = channel();
        let mut release = Vec::new();
        let mut jobs = Vec::new();
        for _ in 0..2 {
            let (sender, receiver) = channel();
            release.push(sender);
            let entered = entered.clone();
            jobs.push(runtime().spawn_blocking(move || {
                entered.send(()).unwrap();
                receiver.recv().unwrap();
            }));
        }
        for _ in 0..2 {
            waiting
                .recv_timeout(std::time::Duration::from_secs(2))
                .unwrap();
        }
        let (completed, completion) = channel();
        runtime().spawn(async move {
            completed.send(()).unwrap();
        });
        let result = completion.recv_timeout(std::time::Duration::from_secs(2));
        for sender in release {
            sender.send(()).unwrap();
        }
        for job in jobs {
            runtime().block_on(job).unwrap();
        }
        result.unwrap();
    }
}
