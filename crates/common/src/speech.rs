#[derive(Default, Clone)]
pub struct SpeechSnapshot {
    pub content: Vec<(String, String)>,
    pub target: Option<(String, String)>,
}

impl SpeechSnapshot {
    #[must_use]
    pub fn announcement(&self, previous: &Self) -> String {
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
