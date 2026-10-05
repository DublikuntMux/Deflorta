//! Music and sound effects on separate mixer tracks (kira).

use std::time::Duration;

use kira::sound::FromFileError;
use kira::sound::static_sound::StaticSoundData;
use kira::sound::streaming::{StreamingSoundData, StreamingSoundHandle};
use kira::track::{TrackBuilder, TrackHandle};
use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Tween};

use crate::assets::Assets;

pub struct Audio {
    _manager: AudioManager<DefaultBackend>,
    music_track: TrackHandle,
    sound_track: TrackHandle,
    music: Option<(String, StreamingSoundHandle<FromFileError>)>,
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

impl Audio {
    /// Returns None when no audio device is available; the game runs silently.
    pub fn new() -> Option<Self> {
        let mut manager = match AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())
        {
            Ok(manager) => manager,
            Err(err) => {
                eprintln!("[deflorta] audio disabled: {err}");
                return None;
            }
        };
        let music_track = manager.add_sub_track(TrackBuilder::new()).ok()?;
        let sound_track = manager.add_sub_track(TrackBuilder::new()).ok()?;
        Some(Audio {
            _manager: manager,
            music_track,
            sound_track,
            music: None,
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
        if let Some((_, mut handle)) = self.music.take() {
            handle.stop(tween(fade_out));
        }
        let Some(file) = file else { return };
        let Some(path) = assets.resolve(file) else {
            return;
        };
        let data = match StreamingSoundData::from_file(&path) {
            Ok(data) => data,
            Err(err) => {
                eprintln!("[deflorta] cannot play music '{file}': {err}");
                return;
            }
        };
        let mut data = data.volume(decibels(volume));
        if looped {
            data = data.loop_region(..);
        }
        if fade_in > 0.0 {
            data = data.fade_in_tween(tween(fade_in));
        }
        match self.music_track.play(data) {
            Ok(handle) => self.music = Some((file.to_owned(), handle)),
            Err(err) => eprintln!("[deflorta] cannot play music '{file}': {err}"),
        }
    }

    pub fn play_sound(&mut self, assets: &Assets, file: &str, volume: f32) {
        let Some(path) = assets.resolve(file) else {
            return;
        };
        let result = StaticSoundData::from_file(&path)
            .map_err(|e| e.to_string())
            .and_then(|data| {
                self.sound_track
                    .play(data.volume(decibels(volume)))
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            });
        if let Err(err) = result {
            eprintln!("[deflorta] cannot play sound '{file}': {err}");
        }
    }

    pub fn set_volume(&mut self, channel: &str, volume: f32) {
        let track = match channel {
            "music" => &mut self.music_track,
            "sound" => &mut self.sound_track,
            _ => return,
        };
        track.set_volume(decibels(volume), tween(0.1));
    }
}
