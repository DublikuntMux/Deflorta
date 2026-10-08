use std::time::Instant;

/// Seconds from `start` to `now`, or zero if `now` is earlier.
#[must_use]
pub fn elapsed_secs(start: Instant, now: Instant) -> f32 {
    now.saturating_duration_since(start).as_secs_f32()
}
