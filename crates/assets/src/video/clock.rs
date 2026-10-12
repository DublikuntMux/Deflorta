use std::sync::Mutex;
use std::time::Instant;

/// Shared playback time, driven by audio when a soundtrack is available.
pub struct PlaybackClock(Mutex<State>);

struct State {
    anchor: Instant,
    elapsed: f64,
    suspended: bool,
    audio: Option<AudioTime>,
}

struct AudioTime {
    previous: f64,
    offset: f64,
    duration: f64,
    looping: bool,
}

impl PlaybackClock {
    #[must_use]
    pub const fn new(now: Instant) -> Self {
        Self(Mutex::new(State {
            anchor: now,
            elapsed: 0.0,
            suspended: false,
            audio: None,
        }))
    }

    #[must_use]
    /// # Panics
    /// Panics if another thread panicked while updating this clock.
    pub fn elapsed(&self, now: Instant) -> f64 {
        let state = self.0.lock().unwrap();
        state.time(now)
    }

    /// # Panics
    /// Panics if another thread panicked while updating this clock.
    pub fn set_suspended(&self, suspended: bool, now: Instant) {
        let mut state = self.0.lock().unwrap();
        state.elapsed = state.time(now);
        state.anchor = now;
        state.suspended = suspended;
    }

    pub(crate) fn attach_audio(&self, position: f64, duration: f64, looping: bool) {
        let now = Instant::now();
        let mut state = self.0.lock().unwrap();
        let elapsed = state.time(now);
        let offset = if looping && duration > 0.0 {
            (elapsed / duration).floor() * duration
        } else {
            0.0
        };
        if !state.suspended {
            state.elapsed = offset + position;
        }
        state.audio = Some(AudioTime {
            previous: position,
            offset,
            duration,
            looping,
        });
    }

    pub(crate) fn update_audio(&self, position: f64, stopped: bool) {
        let mut state = self.0.lock().unwrap();
        if state.suspended {
            return;
        }
        let Some(audio) = &mut state.audio else {
            return;
        };
        if audio.looping && position < audio.previous {
            audio.offset += audio.duration;
        }
        audio.previous = position;
        state.elapsed = audio.offset + position;
        if stopped {
            state.audio = None;
            state.anchor = Instant::now();
        }
    }
}

impl State {
    fn time(&self, now: Instant) -> f64 {
        self.elapsed
            + if self.suspended || self.audio.is_some() {
                0.0
            } else {
                now.saturating_duration_since(self.anchor).as_secs_f64()
            }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn fallback_excludes_suspension() {
        let start = Instant::now();
        let clock = PlaybackClock::new(start);
        clock.set_suspended(true, start + Duration::from_secs(2));
        assert!((clock.elapsed(start + Duration::from_secs(20)) - 2.0).abs() < 1e-9);
        clock.set_suspended(false, start + Duration::from_secs(20));
        assert!((clock.elapsed(start + Duration::from_secs(23)) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn audio_unwraps_loops_and_drives_time() {
        let now = Instant::now();
        let clock = PlaybackClock::new(now);
        clock.attach_audio(0.8, 1.0, true);
        clock.update_audio(0.9, false);
        assert!((clock.elapsed(now + Duration::from_secs(20)) - 0.9).abs() < 1e-9);
        clock.update_audio(0.1, false);
        assert!((clock.elapsed(now) - 1.1).abs() < 1e-9);
        clock.update_audio(0.9, false);
        clock.update_audio(0.1, false);
        assert!((clock.elapsed(now) - 2.1).abs() < 1e-9);
    }

    #[test]
    fn suspension_freezes_audio_and_resume_uses_its_position() {
        let now = Instant::now();
        let clock = PlaybackClock::new(now);
        clock.attach_audio(2.0, 10.0, false);
        clock.set_suspended(true, now);
        clock.update_audio(2.01, false);
        assert!((clock.elapsed(now + Duration::from_secs(60)) - 2.0).abs() < 1e-9);
        clock.set_suspended(false, now + Duration::from_secs(60));
        clock.update_audio(2.02, false);
        assert!((clock.elapsed(now + Duration::from_secs(61)) - 2.02).abs() < 1e-9);
    }

    #[test]
    fn finished_audio_returns_to_fallback_without_a_suspend_gap() {
        let now = Instant::now();
        let clock = PlaybackClock::new(now);
        clock.attach_audio(0.6, 0.6, false);
        clock.update_audio(0.6, true);
        let resumed = Instant::now();
        let before = clock.elapsed(resumed);
        clock.set_suspended(true, resumed);
        clock.set_suspended(true, resumed + Duration::from_secs(5));
        assert!((clock.elapsed(resumed + Duration::from_secs(10)) - before).abs() < 1e-9);
        clock.set_suspended(false, resumed + Duration::from_secs(10));
        assert!((clock.elapsed(resumed + Duration::from_secs(11)) - before - 1.0).abs() < 1e-9);
    }
}
