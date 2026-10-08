#[derive(Clone, Debug)]
pub struct LoadedAsset {
    pub kind: &'static str,
    pub source: String,
    pub state: String,
    pub detail: String,
    pub bytes: Option<u64>,
}

impl LoadedAsset {
    #[must_use]
    pub fn can_unload(&self) -> bool {
        matches!(
            self.kind,
            "Image" | "GPU texture" | "Video" | "Music" | "Voice" | "Sound" | "Video audio"
        )
    }
}

#[derive(Default)]
pub struct GpuStats {
    pub adapter: String,
    pub textures: usize,
    pub texture_bytes: u64,
    pub buffer_bytes: Option<isize>,
    pub all_texture_bytes: Option<isize>,
    pub allocations: Option<(u64, u64)>,
}
