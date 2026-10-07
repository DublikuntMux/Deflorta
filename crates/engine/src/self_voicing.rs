//! Optional system speech, initialized only when a player enables self-voicing.

use log::warn;

#[derive(Default, Clone)]
pub struct SpeechSnapshot {
    pub content: Vec<(String, String)>,
    pub target: Option<(String, String)>,
}

impl SpeechSnapshot {
    fn announcement(&self, previous: &Self) -> String {
        if self.target != previous.target
            && let Some((_, label)) = &self.target
        {
            return label.clone();
        }
        self.content
            .iter()
            .filter(|entry| !previous.content.contains(entry))
            .map(|(_, text)| text.as_str())
            .collect::<Vec<_>>()
            .join(". ")
    }
}

#[derive(Default)]
pub struct SelfVoicing {
    enabled: bool,
    speech: Option<tts::Tts>,
    previous: SpeechSnapshot,
}

impl SelfVoicing {
    pub fn set_enabled(&mut self, enabled: bool) {
        if self.enabled == enabled {
            return;
        }
        self.enabled = enabled;
        self.previous = SpeechSnapshot::default();
        if enabled && self.speech.is_none() {
            self.speech = tts::Tts::default()
                .map_err(|err| warn!("Self-voicing unavailable: {err}"))
                .ok();
        }
        if !enabled
            && let Some(speech) = &mut self.speech
            && let Err(err) = speech.stop()
        {
            warn!("Cannot stop self-voicing: {err}");
        }
    }

    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn update(&mut self, snapshot: SpeechSnapshot) {
        if !self.enabled {
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
