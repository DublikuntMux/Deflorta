use log::warn;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use deflorta_common::speech::SpeechSnapshot;
use deflorta_common::worker::{WakeCallback, WorkerWake};

#[derive(Default)]
pub struct SelfVoicing {
    enabled: bool,
    speech: Option<tts::Tts>,
    previous: SpeechSnapshot,
    speech_retry: Option<(Instant, u8)>,
    initialization: Option<(Receiver<Option<tts::Tts>>, u8)>,
    wake: WorkerWake,
}

impl SelfVoicing {
    pub fn set_waker(&self, callback: WakeCallback) {
        self.wake.set(callback);
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        if self.enabled == enabled {
            return;
        }
        self.enabled = enabled;
        self.previous = SpeechSnapshot::default();
        self.speech_retry = None;
        // Reuse an in-flight attempt if the player toggles off and on during startup.
        if enabled && self.speech.is_none() && self.initialization.is_none() {
            self.initialize(0);
        }
        if !enabled
            && let Some(speech) = &mut self.speech
            && let Err(err) = speech.stop()
        {
            warn!("Cannot stop self-voicing: {err}");
        }
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub const fn initializing(&self) -> bool {
        self.enabled && (self.initialization.is_some() || self.speech_retry.is_some())
    }

    fn initialize(&mut self, attempt: u8) {
        let (sender, receiver) = mpsc::channel();
        let wake = self.wake.clone();
        self.initialization = Some((receiver, attempt));
        if let Err(err) = std::thread::Builder::new()
            .name("self-voicing-init".into())
            .spawn(move || {
                let speech = tts::Tts::default()
                    .map_err(|err| warn!("Self-voicing unavailable: {err}"))
                    .ok();
                let _ = sender.send(speech);
                wake.notify();
            })
        {
            warn!("Cannot start self-voicing initialization: {err}");
        }
    }

    #[must_use]
    pub fn next_retry(&self) -> Option<Instant> {
        self.speech_retry.map(|(due, _)| due)
    }

    /// Slow speech services may not be ready at startup on any platform.
    /// Retry after 2, 4 and 8 seconds, then stop trying.
    /// Returns true when an attempt finishes, including the final failure.
    pub fn retry_initialization(&mut self) -> bool {
        if let Some((receiver, attempt)) = &self.initialization {
            let speech = match receiver.try_recv() {
                Err(TryRecvError::Empty) => return false,
                Ok(speech) => speech,
                Err(TryRecvError::Disconnected) => None,
            };
            let attempt = *attempt;
            self.initialization = None;
            self.speech = speech;
            self.previous = SpeechSnapshot::default();
            if self.enabled && self.speech.is_none() && attempt < 3 {
                self.speech_retry = Some((
                    Instant::now() + Duration::from_secs(2 << attempt),
                    attempt + 1,
                ));
            }
            // Redraw on success and on the final failure to clear the status.
            return true;
        }
        if let Some((due, attempt)) = self.speech_retry
            && Instant::now() >= due
        {
            self.speech_retry = None;
            self.initialize(attempt);
        }
        false
    }

    #[cfg(target_os = "android")]
    pub fn suspend(&mut self) {
        if let Some(speech) = &mut self.speech {
            let _ = speech.stop();
        }
        self.previous = SpeechSnapshot::default();
    }

    pub fn update(&mut self, snapshot: SpeechSnapshot) {
        if !self.enabled || self.speech.is_none() {
            return;
        }
        if snapshot.content.is_empty()
            && !self.previous.content.is_empty()
            && let Some(speech) = &mut self.speech
            && let Err(err) = speech.stop()
        {
            warn!("Cannot stop self-voicing: {err}");
        }
        let text = snapshot.announcement(&self.previous);
        self.previous = snapshot;
        if !text.is_empty()
            && let Some(speech) = &mut self.speech
            && let Err(err) = speech.speak(text, true)
        {
            warn!("Self-voicing failed: {err}");
        }
    }
}

impl Drop for SelfVoicing {
    fn drop(&mut self) {
        if let Some(speech) = &mut self.speech {
            let _ = speech.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_retry_waits_for_deadline_and_disabling_cancels_it() {
        let due = Instant::now() + Duration::from_secs(60);
        let mut voicing = SelfVoicing {
            enabled: true,
            speech: None,
            previous: SpeechSnapshot::default(),
            speech_retry: Some((due, 0)),
            initialization: None,
            wake: WorkerWake::default(),
        };
        assert!(!voicing.retry_initialization());
        assert_eq!(voicing.next_retry(), Some(due));
        voicing.set_enabled(false);
        assert_eq!(voicing.next_retry(), None);
        assert!(!voicing.retry_initialization());
    }

    #[test]
    fn pending_initialization_keeps_content_and_is_reused_after_toggle() {
        let (_sender, receiver) = mpsc::channel();
        let mut voicing = SelfVoicing {
            enabled: true,
            speech: None,
            previous: SpeechSnapshot::default(),
            speech_retry: None,
            initialization: Some((receiver, 0)),
            wake: WorkerWake::default(),
        };
        assert!(voicing.initializing());
        assert!(!voicing.retry_initialization());
        voicing.update(SpeechSnapshot {
            content: vec![("line".into(), "Hello".into())],
            target: None,
        });
        assert_eq!(voicing.previous.content, [] as [(String, String); 0]);
        voicing.set_enabled(false);
        assert!(!voicing.initializing());
        voicing.set_enabled(true);
        assert!(voicing.initializing());
        assert!(!voicing.retry_initialization());
    }

    #[test]
    fn failed_initialization_shows_status_through_retries_then_clears_it() {
        let mut voicing = SelfVoicing {
            enabled: true,
            speech: None,
            previous: SpeechSnapshot::default(),
            speech_retry: None,
            initialization: None,
            wake: WorkerWake::default(),
        };
        for attempt in 0..=3 {
            let (sender, receiver) = mpsc::channel();
            voicing.speech_retry = None;
            voicing.initialization = Some((receiver, attempt));
            sender.send(None).unwrap();
            let before = Instant::now();
            assert!(voicing.retry_initialization());
            assert_eq!(voicing.initializing(), attempt < 3);
            if attempt < 3 {
                let (due, next_attempt) = voicing.speech_retry.unwrap();
                assert!(due >= before + Duration::from_secs(2 << attempt));
                assert_eq!(next_attempt, attempt + 1);
            }
        }
    }

    #[test]
    fn disabling_during_initialization_does_not_schedule_a_retry() {
        let (sender, receiver) = mpsc::channel();
        let mut voicing = SelfVoicing {
            enabled: true,
            speech: None,
            previous: SpeechSnapshot::default(),
            speech_retry: None,
            initialization: Some((receiver, 0)),
            wake: WorkerWake::default(),
        };
        voicing.set_enabled(false);
        sender.send(None).unwrap();
        assert!(voicing.retry_initialization());
        assert!(!voicing.initializing());
        assert_eq!(voicing.next_retry(), None);
    }

    #[test]
    fn announces_changes_and_focus_without_repeating_unchanged_text() {
        let first = SpeechSnapshot {
            content: vec![
                ("speaker".into(), "Eileen".into()),
                ("line".into(), "Hello world".into()),
            ],
            target: None,
        };
        assert_eq!(
            first.announcement(&SpeechSnapshot::default()),
            "Eileen. Hello world"
        );
        assert_eq!(first.announcement(&first), "");
        let mut next = first.clone();
        next.content[1].1 = "Goodbye".into();
        assert_eq!(next.announcement(&first), "Goodbye");
        next.target = Some(("save".into(), "Save".into()));
        assert_eq!(next.announcement(&first), "Save");
        assert_eq!(next.announcement(&next), "");
        let mut slider = next.clone();
        slider.target = Some(("save".into(), "Volume, 0.5".into()));
        assert_eq!(slider.announcement(&next), "Volume, 0.5");
    }
}
