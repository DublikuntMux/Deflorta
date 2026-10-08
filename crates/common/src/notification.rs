use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

pub type NotificationId = u64;

#[must_use]
pub fn next_notification_id() -> NotificationId {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NotificationState {
    #[default]
    Info,
    Loading,
    Success,
    Error,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct NotificationOptions {
    pub state: NotificationState,
    /// Seconds until dismissal; None keeps the notification until explicitly dismissed.
    pub duration: Option<f64>,
}

impl Default for NotificationOptions {
    fn default() -> Self {
        Self {
            state: NotificationState::Info,
            duration: Some(2.0),
        }
    }
}

impl NotificationOptions {
    ///
    /// # Errors
    ///
    /// Returns an error for a negative, non-finite, or unrepresentable duration.
    pub fn deadline(self, now: Instant) -> Result<Option<Instant>, &'static str> {
        self.duration
            .map(|seconds| {
                let duration = Duration::try_from_secs_f64(seconds)
                    .map_err(|_| "Notification duration must be a finite, non-negative number")?;
                now.checked_add(duration)
                    .ok_or("Notification duration is too large")
            })
            .transpose()
    }
}
