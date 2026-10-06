//! Music, sound effects, voice and video soundtracks on separate mixer
//! tracks (kira).

mod webm;

use std::collections::HashMap;
use std::time::Duration;

use kira::sound::FromFileError;
use kira::sound::static_sound::StaticSoundData;
use kira::sound::streaming::{StreamingSoundData, StreamingSoundHandle};
use kira::track::{TrackBuilder, TrackHandle};
use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Tween};

use log::{debug, info, warn};

use crate::assets::Assets;
use crate::files::GameFiles;

type Stream = StreamingSoundHandle<FromFileError>;

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
}

fn tween(seconds: f32) -> Tween {
    Tween {
        duration: Duration::from_secs_f32(seconds.max(0.0)),
        ..Default::default()
    }
}

/// Converts a linear amplitude in [0, 1] to decibels.
fn decibels(volume: f32) -> Decibels {
    if volume <= 0.001 {
        Decibels::SILENCE
    } else {
        Decibels(20.0 * volume.log10())
    }
}

fn stream(
    assets: &Assets,
    file: &str,
    looped: bool,
    volume: f32,
    fade_in: f32,
) -> Option<StreamingSoundData<FromFileError>> {
    let path = assets.game_path(file)?;
    let mut data = match open_stream(assets.files(), &path) {
        Ok(data) => data.volume(decibels(volume)),
        Err(err) => {
            warn!("Cannot play '{file}': {err}");
            return None;
        }
    };
    if looped {
        data = data.loop_region(..);
    }
    if fade_in > 0.0 {
        data = data.fade_in_tween(tween(fade_in));
    }
    Some(data)
}

impl Audio {
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
        if let (Some((current, _)), Some(file)) = (&self.music, file)
            && current == file
        {
            return;
        }
        if let Some((previous, mut handle)) = self.music.take() {
            debug!("Stopping music '{previous}' (fade out {fade_out}s)");
            handle.stop(tween(fade_out));
        }
        let Some(file) = file else { return };
        let Some(data) = stream(assets, file, looped, volume, fade_in) else {
            return;
        };
        match self.music_track.play(data) {
            Ok(handle) => {
                info!("Playing music '{file}' (loop: {looped}, fade in {fade_in}s)");
                self.music = Some((file.to_owned(), handle));
            }
            Err(err) => warn!("Cannot play music '{file}': {err}"),
        }
    }

    pub fn play_sound(&mut self, assets: &Assets, file: &str, volume: f32) {
        let Some(path) = assets.game_path(file) else {
            return;
        };
        let result = assets
            .files()
            .open_file(&path)
            .map_err(|e| e.to_string())
            .and_then(|reader| {
                StaticSoundData::from_media_source(reader).map_err(|e| e.to_string())
            })
            .and_then(|data| {
                self.sound_track
                    .play(data.volume(decibels(volume)))
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            });
        match result {
            Ok(()) => debug!("Playing sound '{file}'"),
            Err(err) => warn!("Cannot play sound '{file}': {err}"),
        }
    }

    /// Plays a voice line, cutting off the previous one.
    pub fn play_voice(&mut self, assets: &Assets, file: Option<&str>) {
        if let Some(mut handle) = self.voice.take() {
            handle.stop(tween(0.05));
        }
        let Some(file) = file else { return };
        let Some(data) = stream(assets, file, false, 1.0, 0.0) else {
            return;
        };
        match self.voice_track.play(data) {
            Ok(handle) => {
                debug!("Playing voice '{file}'");
                self.voice = Some(handle);
            }
            Err(err) => warn!("Cannot play voice '{file}': {err}"),
        }
    }

    /// Keeps the soundtracks of on-screen videos playing; stops the others.
    pub fn sync_videos<'a>(
        &mut self,
        assets: &Assets,
        playing: impl Iterator<Item = (&'a str, bool)>,
    ) {
        let playing: HashMap<&str, bool> = playing.collect();
        self.videos.retain(|src, handle| {
            let keep = playing.contains_key(src.as_str());
            if !keep {
                debug!("Stopping soundtrack of video '{src}'");
                handle.stop(tween(0.1));
            }
            keep
        });
        for (src, looped) in playing {
            if self.videos.contains_key(src) {
                continue;
            }
            // Videos without an audio track simply play silently.
            let Some(path) = assets.game_path(src) else {
                continue;
            };
            let data = match open_stream(assets.files(), &path) {
                Ok(data) => data,
                Err(err) => {
                    debug!("Video '{src}' has no playable soundtrack: {err}");
                    continue;
                }
            };
            let data = if looped { data.loop_region(..) } else { data };
            if let Ok(handle) = self.music_track.play(data) {
                debug!("Playing soundtrack of video '{src}'");
                self.videos.insert(src.to_owned(), handle);
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
