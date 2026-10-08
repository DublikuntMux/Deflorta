mod loader;
mod webm;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use kira::sound::FromFileError;
use kira::sound::streaming::StreamingSoundData;
use kira::track::{TrackBuilder, TrackHandle};
use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Tween};

use log::{debug, info, warn};

use crate::assets::Assets;
use crate::files::GameFiles;
use crate::worker::WakeCallback;
use loader::{AudioLoader, LoadKind, LoadedSound, StreamHandle};

type Stream = StreamHandle;

fn open_stream(
    files: &GameFiles,
    path: &str,
) -> Result<StreamingSoundData<FromFileError>, FromFileError> {
    if crate::video::is_webm(path) {
        Ok(StreamingSoundData::from_decoder(webm::Decoder::open(
            files, path,
        )?))
    } else {
        StreamingSoundData::from_media_source(files.open_file(path)?)
    }
}

pub struct Audio {
    _manager: AudioManager<DefaultBackend>,
    music_track: TrackHandle,
    sound_track: TrackHandle,
    voice_track: TrackHandle,
    music: Option<(String, Stream)>,
    voice: Option<Stream>,
    videos: HashMap<String, Stream>,
    loader: AudioLoader,
    music_request: Option<u64>,
    voice_request: Option<u64>,
    video_requests: HashMap<String, u64>,
    #[cfg(feature = "dev-console")]
    voice_source: Option<String>,
    #[cfg(feature = "dev-console")]
    sounds: Vec<(String, kira::sound::static_sound::StaticSoundHandle)>,
}

fn tween(seconds: f32) -> Tween {
    Tween {
        duration: Duration::from_secs_f32(seconds.max(0.0)),
        ..Default::default()
    }
}

fn decibels(volume: f32) -> Decibels {
    if volume <= 0.001 {
        Decibels::SILENCE
    } else {
        Decibels(20.0 * volume.log10())
    }
}

impl Audio {
    pub fn set_waker(&self, callback: WakeCallback) {
        self.loader.set_waker(callback);
    }

    #[cfg(target_os = "android")]
    pub fn set_suspended(&mut self, suspended: bool) {
        for track in [
            &mut self.music_track,
            &mut self.sound_track,
            &mut self.voice_track,
        ] {
            if suspended {
                track.pause(tween(0.0));
            } else {
                track.resume(tween(0.0));
            }
        }
    }

    /// Returns None when no audio device is available; the game runs silently.
    pub fn new() -> Option<Self> {
        let mut manager = match AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())
        {
            Ok(manager) => manager,
            Err(err) => {
                warn!("No audio device, running silently: {err}");
                return None;
            }
        };
        let music_track = manager.add_sub_track(TrackBuilder::new()).ok()?;
        let sound_track = manager.add_sub_track(TrackBuilder::new()).ok()?;
        let voice_track = manager.add_sub_track(TrackBuilder::new()).ok()?;
        info!("Audio ready (music, sound and voice tracks)");

        Some(Self {
            _manager: manager,
            music_track,
            sound_track,
            voice_track,
            music: None,
            voice: None,
            videos: HashMap::new(),
            loader: AudioLoader::new(),
            music_request: None,
            voice_request: None,
            video_requests: HashMap::new(),
            #[cfg(feature = "dev-console")]
            voice_source: None,
            #[cfg(feature = "dev-console")]
            sounds: Vec::new(),
        })
    }

    pub fn play_music(
        &mut self,
        assets: &Assets,
        file: Option<&str>,
        looped: bool,
        volume: f32,
        fade_in: f32,
        fade_out: f32,
    ) {
        if file.is_some_and(|file| {
            self.music
                .as_ref()
                .is_some_and(|(current, _)| current == file)
                || self.music_request.and_then(|id| self.loader.source(id)) == Some(file)
        }) {
            return;
        }
        if let Some(id) = self.music_request.take() {
            self.loader.cancel(id);
        }
        if let Some((previous, handle)) = self.music.take() {
            debug!("Stopping music '{previous}' (fade out {fade_out}s)");
            handle.stop(tween(fade_out));
        }
        let Some(file) = file else { return };
        let Some(path) = assets.game_path(file) else {
            return;
        };
        self.music_request = self.loader.request(
            assets.files().clone(),
            path,
            file,
            LoadKind::Music {
                looped,
                volume,
                fade_in,
            },
        );
    }

    pub fn play_sound(&mut self, assets: &Assets, file: &str, volume: f32) {
        #[cfg(feature = "dev-console")]
        self.sounds
            .retain(|(_, sound)| sound.state() != kira::sound::PlaybackState::Stopped);
        let Some(path) = assets.game_path(file) else {
            return;
        };
        self.loader.request(
            assets.files().clone(),
            path,
            file,
            LoadKind::Sound { volume },
        );
    }

    pub fn play_voice(&mut self, assets: &Assets, file: Option<&str>) {
        if let Some(id) = self.voice_request.take() {
            self.loader.cancel(id);
        }
        #[cfg(feature = "dev-console")]
        {
            self.voice_source = None;
        }
        if let Some(handle) = self.voice.take() {
            handle.stop(tween(0.05));
        }
        let Some(file) = file else { return };
        let Some(path) = assets.game_path(file) else {
            return;
        };
        self.voice_request =
            self.loader
                .request(assets.files().clone(), path, file, LoadKind::Voice);
    }

    pub fn sync_videos<'a>(
        &mut self,
        assets: &Assets,
        playing: impl Iterator<Item = (&'a str, bool, Instant)>,
    ) {
        let playing: HashMap<&str, (bool, Instant)> = playing
            .map(|(source, looped, started)| (source, (looped, started)))
            .collect();
        self.video_requests.retain(|src, id| {
            if playing.contains_key(src.as_str()) {
                true
            } else {
                self.loader.cancel(*id);
                false
            }
        });
        self.videos.retain(|src, handle| {
            let keep = playing.contains_key(src.as_str());
            if !keep {
                debug!("Stopping soundtrack of video '{src}'");
                handle.stop(tween(0.1));
            }
            keep
        });
        for (src, (looped, started)) in playing {
            if self.videos.contains_key(src) || self.video_requests.contains_key(src) {
                continue;
            }
            let Some(path) = assets.game_path(src) else {
                continue;
            };
            if let Some(id) = self.loader.request(
                assets.files().clone(),
                path,
                src,
                LoadKind::Video { looped, started },
            ) {
                self.video_requests.insert(src.to_owned(), id);
            }
        }
    }

    pub fn poll(&mut self) {
        for (request, data) in self.loader.poll() {
            let source = &request.source;
            match request.kind {
                LoadKind::Music { .. } => self.music_request = None,
                LoadKind::Voice => self.voice_request = None,
                LoadKind::Video { .. } => {
                    self.video_requests.remove(source);
                }
                LoadKind::Sound { .. } => {}
            }
            let data = match data {
                Ok(data) => data,
                Err(error) => {
                    if matches!(request.kind, LoadKind::Video { .. }) {
                        debug!("Video '{source}' has no playable soundtrack: {error}");
                    } else {
                        warn!("Cannot load audio '{source}': {error}");
                    }
                    continue;
                }
            };
            match (request.kind, data) {
                (
                    LoadKind::Music {
                        looped, fade_in, ..
                    },
                    LoadedSound::Stream(data),
                ) => match self.music_track.play(data) {
                    Ok(handle) => {
                        info!("Playing music '{source}' (loop: {looped}, fade in {fade_in}s)");
                        self.music = Some((source.clone(), handle));
                    }
                    Err(error) => warn!("Cannot play music '{source}': {error}"),
                },
                (LoadKind::Voice, LoadedSound::Stream(data)) => match self.voice_track.play(data) {
                    Ok(handle) => {
                        debug!("Playing voice '{source}'");
                        self.voice = Some(handle);
                        #[cfg(feature = "dev-console")]
                        {
                            self.voice_source = Some(source.clone());
                        }
                    }
                    Err(error) => warn!("Cannot play voice '{source}': {error}"),
                },
                (LoadKind::Sound { volume }, LoadedSound::Static(data)) => {
                    match self.sound_track.play((*data).volume(decibels(volume))) {
                        Ok(handle) => {
                            debug!("Playing sound '{source}'");
                            #[cfg(feature = "dev-console")]
                            self.sounds.push((source.clone(), handle));
                            #[cfg(not(feature = "dev-console"))]
                            drop(handle);
                        }
                        Err(error) => warn!("Cannot play sound '{source}': {error}"),
                    }
                }
                (LoadKind::Video { .. }, LoadedSound::Stream(data)) => {
                    if let Ok(handle) = self.music_track.play(data) {
                        debug!("Playing soundtrack of video '{source}'");
                        self.videos.insert(source.clone(), handle);
                    }
                }
                _ => unreachable!("audio loader returns the requested sound type"),
            }
        }
    }

    pub fn set_volume(&mut self, channel: &str, volume: f32) {
        let track = match channel {
            "music" => &mut self.music_track,
            "sound" => &mut self.sound_track,
            "voice" => &mut self.voice_track,
            _ => return,
        };
        debug!("{channel} volume {volume:.2}");
        track.set_volume(decibels(volume), tween(0.1));
    }

    pub fn collect_finished(&mut self) {
        use kira::sound::PlaybackState;
        if self
            .music
            .as_ref()
            .is_some_and(|(_, handle)| handle.state() == PlaybackState::Stopped)
        {
            self.music = None;
        }
        if self
            .voice
            .as_ref()
            .is_some_and(|handle| handle.state() == PlaybackState::Stopped)
        {
            self.voice = None;
            #[cfg(feature = "dev-console")]
            {
                self.voice_source = None;
            }
        }
        // Keep stopped video IDs until their UI node leaves, so sync_videos does
        // not restart a soundtrack that already reached its end.
        #[cfg(feature = "dev-console")]
        self.sounds
            .retain(|(_, sound)| sound.state() != PlaybackState::Stopped);
    }

    #[cfg(feature = "dev-console")]
    pub fn unload(&mut self, source: &str) -> bool {
        let mut released = self.loader.cancel_source(source);
        if self
            .music_request
            .is_some_and(|id| self.loader.source(id).is_none())
        {
            self.music_request = None;
        }
        if self
            .voice_request
            .is_some_and(|id| self.loader.source(id).is_none())
        {
            self.voice_request = None;
        }
        self.video_requests
            .retain(|_, id| self.loader.source(*id).is_some());
        if self.music.as_ref().is_some_and(|(src, _)| src == source) {
            let (_, handle) = self.music.take().unwrap();
            handle.stop(tween(0.0));
            released = true;
        }
        if self.voice_source.as_deref() == Some(source) {
            if let Some(handle) = self.voice.take() {
                handle.stop(tween(0.0));
                released = true;
            }
            self.voice_source = None;
        }
        if let Some(handle) = self.videos.remove(source) {
            handle.stop(tween(0.0));
            released = true;
        }
        self.sounds.retain_mut(|(src, handle)| {
            if src != source {
                return true;
            }
            handle.stop(tween(0.0));
            released = true;
            false
        });
        released
    }

    #[cfg(feature = "dev-console")]
    pub fn loaded_assets(&self) -> Vec<crate::dev_console::diagnostics::LoadedAsset> {
        use crate::dev_console::diagnostics::LoadedAsset;
        use kira::sound::PlaybackState;
        let mut assets = self.loader.loaded_assets();
        let mut add = |kind, source: &str, state| {
            if state != PlaybackState::Stopped {
                assets.push(LoadedAsset {
                    kind,
                    source: source.into(),
                    state: format!("{state:?}"),
                    detail: "Streamed".into(),
                    bytes: None,
                });
            }
        };
        if let Some((source, handle)) = &self.music {
            add("Music", source, handle.state());
        }
        if let (Some(source), Some(handle)) = (&self.voice_source, &self.voice) {
            add("Voice", source, handle.state());
        }
        for (source, handle) in &self.videos {
            add("Video audio", source, handle.state());
        }
        for (source, handle) in &self.sounds {
            if handle.state() != PlaybackState::Stopped {
                assets.push(LoadedAsset {
                    kind: "Sound",
                    source: source.clone(),
                    state: format!("{:?}", handle.state()),
                    detail: "Decoded sound effect".into(),
                    bytes: None,
                });
            }
        }
        assets
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_files(dir: &str) -> GameFiles {
        GameFiles::directory(&crate::workspace_dir().join(dir)).unwrap()
    }

    #[test]
    fn decodes_movie_soundtrack() {
        let files = workspace_files("game");
        assert!(open_stream(&files, "movies/intro.mp4").is_ok());
    }

    #[test]
    fn streams_webm_soundtrack_and_rewinds() {
        use kira::sound::streaming::Decoder as _;

        let files = workspace_files("tests/fixtures");
        for codec in ["vp8", "vp9"] {
            let path = format!("{codec}-vorbis.webm");
            assert!(open_stream(&files, &path).is_ok());
            let mut decoder = webm::Decoder::open(&files, &path).unwrap();
            assert_eq!(decoder.sample_rate(), 48_000);
            assert!((28_800..31_200).contains(&decoder.num_frames()));
            let first = decoder.decode().unwrap();
            assert_ne!(first.len(), 0);
            let mut sample_count = first.len();
            let mut audible = first.iter().any(|frame| frame.left.abs() > 0.01);
            let mut last = first.clone();
            loop {
                let frames = decoder.decode().unwrap();
                if frames.is_empty() {
                    break;
                }
                sample_count += frames.len();
                audible |= frames.iter().any(|frame| frame.left.abs() > 0.01);
                last = frames;
            }
            assert!(audible);
            assert!(sample_count >= decoder.num_frames());
            if codec == "vp9" {
                // This stereo soundtrack ends before the video: fill its tail with silence.
                assert!(last.iter().all(|frame| *frame == kira::Frame::ZERO));
            }
            assert!(
                (28_000..31_200).contains(&sample_count),
                "decoded {sample_count} samples"
            );
            assert_eq!(decoder.seek(0).unwrap(), 0);
            assert_eq!(decoder.decode().unwrap(), first);
            assert!(decoder.seek(14_400).unwrap() <= 14_400);
            assert_ne!(decoder.decode().unwrap().len(), 0);
        }
    }
}
