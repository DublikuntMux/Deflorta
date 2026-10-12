use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, Weak};
use std::time::Instant;

use kira::sound::static_sound::StaticSoundData;
use kira::sound::streaming::{StreamingSoundData, StreamingSoundHandle};
use kira::sound::{FromFileError, Sound, SoundData};
use log::warn;

use crate::files::GameFiles;
use crate::video::PlaybackClock;
use crate::worker::{WakeCallback, WorkerWake};

#[derive(Clone)]
pub(super) enum LoadKind {
    Music {
        looped: bool,
        volume: f32,
        fade_in: f32,
    },
    Voice,
    Sound {
        volume: f32,
    },
    Video {
        looped: bool,
        clock: Arc<PlaybackClock>,
    },
}

pub(super) enum LoadedSound {
    Static(Box<StaticSoundData>),
    Stream(PreparedStream),
}

/// Kira seeks the decoder in `into_sound`. Prepare it on the loader thread;
/// handing this value to a track only transfers the prepared sound and handle.
pub(super) struct PreparedStream {
    sound: ManagedStream,
}

impl PreparedStream {
    pub fn activate_clock(&self) {
        if let Some((clock, duration, looping)) = &self.sound.clock {
            let elapsed = clock.elapsed(Instant::now());
            let position = if *looping && *duration > 0.0 {
                elapsed % duration
            } else {
                elapsed.min(*duration)
            };
            self.sound.handle.handle.lock().unwrap().seek_to(position);
        }
    }
}

#[derive(Clone)]
pub(super) struct StreamHandle {
    handle: Arc<Mutex<StreamingSoundHandle<FromFileError>>>,
    video_clocks: Option<Arc<Mutex<Vec<ClockFollower>>>>,
}

struct ClockFollower {
    clock: Weak<PlaybackClock>,
    started: bool,
}

impl StreamHandle {
    pub fn state(&self) -> kira::sound::PlaybackState {
        self.handle.lock().unwrap().state()
    }

    pub fn stop(&self, tween: kira::Tween) {
        self.handle.lock().unwrap().stop(tween);
    }

    fn position(&self) -> f64 {
        self.handle.lock().unwrap().position()
    }

    pub fn add_video_clock(&self, clock: &Arc<PlaybackClock>) {
        if let Some(clocks) = &self.video_clocks {
            let mut clocks = clocks.lock().unwrap();
            let weak = Arc::downgrade(clock);
            if !clocks.iter().any(|follower| follower.clock.ptr_eq(&weak)) {
                clocks.push(ClockFollower {
                    clock: weak,
                    started: false,
                });
            }
        }
    }
}

struct ManagedStream {
    inner: Box<dyn Sound>,
    handle: StreamHandle,
    clock: Option<(Arc<PlaybackClock>, f64, bool)>,
}

impl Sound for ManagedStream {
    fn on_start_processing(&mut self) {
        self.inner.on_start_processing();
    }

    fn process(&mut self, out: &mut [kira::Frame], dt: f64, info: &kira::info::Info) {
        self.inner.process(out, dt, info);
        if let Some((_, duration, looping)) = &self.clock
            && let Some(clocks) = &self.handle.video_clocks
        {
            let position = self.handle.position();
            let stopped = self.inner.finished();
            clocks.lock().unwrap().retain_mut(|follower| {
                let Some(clock) = follower.clock.upgrade() else {
                    return false;
                };
                if !follower.started {
                    clock.attach_audio(position, *duration, *looping);
                    follower.started = true;
                }
                clock.update_audio(position, stopped);
                true
            });
        }
    }

    fn finished(&self) -> bool {
        self.inner.finished()
    }
}

impl SoundData for PreparedStream {
    type Error = FromFileError;
    type Handle = StreamHandle;

    fn into_sound(self) -> Result<(Box<dyn Sound>, Self::Handle), Self::Error> {
        let handle = self.sound.handle.clone();
        Ok((Box::new(self.sound), handle))
    }
}

impl Drop for ManagedStream {
    fn drop(&mut self) {
        // Kira's scheduler stops when the sound processes a stop command.
        // A cancelled prepared stream never reaches the audio renderer,
        // so process that command here before dropping its sound.
        self.handle.stop(super::tween(0.0));
        self.inner.on_start_processing();
        // These streams use constant parameters and no clocks or
        // modulators, so an empty resource context can finish the
        // zero-duration stop without rendering any audio frames.
        self.inner
            .process(&mut [], 0.0, &kira::info::MockInfoBuilder::new().build());
    }
}

fn prepare_stream(
    mut data: StreamingSoundData<FromFileError>,
    kind: LoadKind,
) -> Result<PreparedStream, String> {
    let mut playback_clock = None;
    match kind {
        LoadKind::Music {
            looped,
            volume,
            fade_in,
        } => {
            data = data.volume(super::decibels(volume));
            if looped {
                data = data.loop_region(..);
            }
            if fade_in > 0.0 {
                data = data.fade_in_tween(super::tween(fade_in));
            }
        }
        LoadKind::Video { looped, clock } => {
            let elapsed = clock.elapsed(Instant::now());
            let duration = data.duration().as_secs_f64();
            let position = if looped && duration > 0.0 {
                elapsed % duration
            } else {
                elapsed.min(duration)
            };
            data = data.start_position(position);
            if looped {
                data = data.loop_region(..);
            }
            playback_clock = Some((clock, duration, looped));
        }
        LoadKind::Voice => {}
        LoadKind::Sound { .. } => unreachable!("sound effects use static data"),
    }
    let (sound, handle) = data.into_sound().map_err(|e| e.to_string())?;
    let video_clocks = playback_clock.as_ref().map(|(clock, _, _)| {
        Arc::new(Mutex::new(vec![ClockFollower {
            clock: Arc::downgrade(clock),
            started: false,
        }]))
    });
    Ok(PreparedStream {
        sound: ManagedStream {
            inner: sound,
            handle: StreamHandle {
                handle: Arc::new(Mutex::new(handle)),
                video_clocks,
            },
            clock: playback_clock,
        },
    })
}

pub(super) struct LoadRequest {
    pub source: String,
    pub kind: LoadKind,
    cancelled: Arc<AtomicBool>,
}

impl Drop for LoadRequest {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

struct Job {
    id: u64,
    files: GameFiles,
    path: String,
    kind: LoadKind,
    cancelled: Arc<AtomicBool>,
}

type Completion = (u64, Result<LoadedSound, String>);

pub(super) struct AudioLoader {
    jobs: Sender<Job>,
    results: Receiver<Completion>,
    pending: HashMap<u64, LoadRequest>,
    next_id: u64,
    wake: WorkerWake,
}

impl AudioLoader {
    pub fn new() -> Self {
        let (jobs, receiver) = channel::<Job>();
        let (sender, results) = channel();
        let wake = WorkerWake::default();
        let worker_wake = wake.clone();
        std::thread::Builder::new()
            .name("audio loader".into())
            .spawn(move || {
                while let Ok(job) = receiver.recv() {
                    if job.cancelled.load(Ordering::Relaxed) {
                        continue;
                    }
                    let data = if matches!(job.kind, LoadKind::Sound { .. }) {
                        job.files
                            .open_file(&job.path)
                            .map_err(|e| e.to_string())
                            .and_then(|reader| {
                                StaticSoundData::from_media_source(reader)
                                    .map_err(|e| e.to_string())
                            })
                            .map(|data| LoadedSound::Static(Box::new(data)))
                    } else {
                        super::open_stream(&job.files, &job.path)
                            .map_err(|e| e.to_string())
                            .and_then(|data| prepare_stream(data, job.kind))
                            .map(LoadedSound::Stream)
                    };
                    if job.cancelled.load(Ordering::Relaxed) {
                        continue;
                    }
                    if sender.send((job.id, data)).is_err() {
                        return;
                    }
                    worker_wake.notify();
                }
            })
            .expect("spawn audio loader");
        Self {
            jobs,
            results,
            pending: HashMap::new(),
            next_id: 0,
            wake,
        }
    }

    pub fn set_waker(&self, callback: WakeCallback) {
        self.wake.set(callback);
    }

    pub fn request(
        &mut self,
        files: GameFiles,
        path: String,
        source: &str,
        kind: LoadKind,
    ) -> Option<u64> {
        let id = self.next_id;
        self.next_id += 1;
        let cancelled = Arc::new(AtomicBool::new(false));
        let job = Job {
            id,
            files,
            path,
            kind: kind.clone(),
            cancelled: cancelled.clone(),
        };
        if self.jobs.send(job).is_err() {
            warn!("Cannot load audio '{source}': audio loader stopped");
            return None;
        }
        self.pending.insert(
            id,
            LoadRequest {
                source: source.into(),
                kind,
                cancelled,
            },
        );
        Some(id)
    }

    pub fn source(&self, id: u64) -> Option<&str> {
        self.pending.get(&id).map(|request| request.source.as_str())
    }

    pub fn cancel(&mut self, id: u64) {
        self.pending.remove(&id);
    }

    #[cfg(feature = "dev-console")]
    pub fn cancel_source(&mut self, source: &str) -> bool {
        let before = self.pending.len();
        self.pending.retain(|_, request| request.source != source);
        before != self.pending.len()
    }

    #[cfg(feature = "dev-console")]
    pub fn loaded_assets(&self) -> Vec<deflorta_common::diagnostics::LoadedAsset> {
        self.pending
            .values()
            .map(|request| deflorta_common::diagnostics::LoadedAsset {
                kind: match request.kind {
                    LoadKind::Music { .. } => "Music",
                    LoadKind::Voice => "Voice",
                    LoadKind::Sound { .. } => "Sound",
                    LoadKind::Video { .. } => "Video audio",
                },
                source: request.source.clone(),
                state: "Loading".into(),
                detail: String::new(),
                bytes: None,
            })
            .collect()
    }

    pub fn poll(&mut self) -> Vec<(LoadRequest, Result<LoadedSound, String>)> {
        let mut loaded = Vec::new();
        while let Ok((id, data)) = self.results.try_recv() {
            if let Some(request) = self.pending.remove(&id) {
                loaded.push((request, data));
            }
        }
        loaded
    }
}

impl Drop for AudioLoader {
    fn drop(&mut self) {
        self.pending.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn worker_loads_sound_and_prepares_stream_before_notifying() {
        let files = GameFiles::directory(&crate::workspace_dir().join("tests/fixtures")).unwrap();
        let mut loader = AudioLoader::new();
        let (sender, receiver) = channel();
        loader.set_waker(Arc::new(move || {
            let _ = sender.send(());
        }));
        receiver.try_recv().unwrap();
        loader
            .request(
                files.clone(),
                "chime.ogg".into(),
                "chime",
                LoadKind::Sound { volume: 0.5 },
            )
            .unwrap();
        receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        let mut completions = loader.poll();
        assert_eq!(completions.len(), 1);
        let (request, result) = completions.pop().unwrap();
        assert_eq!(request.source, "chime");
        let LoadedSound::Static(sound) = result.unwrap_or_else(|e| panic!("{e}")) else {
            panic!("expected a sound effect")
        };
        assert!(sound.frames.iter().any(|frame| frame.left.abs() > 0.01));
        loader
            .request(
                files.clone(),
                "theme.ogg".into(),
                "theme",
                LoadKind::Music {
                    looped: true,
                    volume: 1.0,
                    fade_in: 0.1,
                },
            )
            .unwrap();
        receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        let (_, result) = loader.poll().pop().unwrap();
        assert!(matches!(result, Ok(LoadedSound::Stream(_))));
        loader
            .request(files, "missing.wav".into(), "missing", LoadKind::Voice)
            .unwrap();
        receiver.recv_timeout(Duration::from_secs(5)).unwrap();
        let (_, result) = loader.poll().pop().unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn cancellation_discards_already_published_results_even_for_the_same_source() {
        let (jobs, requests) = channel();
        let (sender, results) = channel();
        let mut loader = AudioLoader {
            jobs,
            results,
            pending: HashMap::new(),
            next_id: 0,
            wake: WorkerWake::default(),
        };
        let files = GameFiles::directory(&crate::workspace_dir().join("game")).unwrap();
        let old = loader
            .request(
                files.clone(),
                "audio/voice_hello.wav".into(),
                "voice",
                LoadKind::Voice,
            )
            .unwrap();
        let job = requests.recv().unwrap();
        sender.send((old, Err("old completion".into()))).unwrap();
        loader.cancel(old);
        assert!(job.cancelled.load(Ordering::Relaxed));
        let new = loader
            .request(
                files,
                "audio/voice_hello.wav".into(),
                "voice",
                LoadKind::Voice,
            )
            .unwrap();
        assert_ne!(old, new);
        sender.send((new, Err("new completion".into()))).unwrap();
        let completions = loader.poll();
        assert_eq!(completions.len(), 1);
        assert_eq!(completions[0].0.source, "voice");
        assert!(matches!(&completions[0].1, Err(message) if message == "new completion"));
        assert!(loader.pending.is_empty());
    }

    #[test]
    fn video_stream_preparation_accounts_for_time_spent_loading() {
        let files = GameFiles::directory(&crate::workspace_dir().join("tests/fixtures")).unwrap();
        let data = super::super::open_stream(&files, "vp8-vorbis.webm").unwrap();
        let started = Instant::now()
            .checked_sub(Duration::from_millis(250))
            .unwrap();
        let prepared = prepare_stream(
            data,
            LoadKind::Video {
                looped: false,
                clock: Arc::new(PlaybackClock::new(started)),
            },
        )
        .unwrap();
        assert!(prepared.sound.handle.position() >= 0.25);
    }

    #[test]
    fn video_stream_preparation_excludes_time_suspended() {
        let files = GameFiles::directory(&crate::workspace_dir().join("tests/fixtures")).unwrap();
        let data = super::super::open_stream(&files, "vp9-vorbis.webm").unwrap();
        let now = Instant::now();
        let clock = Arc::new(PlaybackClock::new(
            now.checked_sub(Duration::from_secs(10)).unwrap(),
        ));
        clock.set_suspended(true, now.checked_sub(Duration::from_millis(9750)).unwrap());
        let prepared = prepare_stream(
            data,
            LoadKind::Video {
                looped: false,
                clock,
            },
        )
        .unwrap();
        assert!((prepared.sound.handle.position() - 0.25).abs() < 0.001);
    }

    #[test]
    fn a_shared_soundtrack_drives_each_video_clock_and_releases_removed_players() {
        let files = GameFiles::directory(&crate::workspace_dir().join("tests/fixtures")).unwrap();
        let now = Instant::now();
        let primary = Arc::new(PlaybackClock::new(
            now.checked_sub(Duration::from_millis(250)).unwrap(),
        ));
        let mut prepared = prepare_stream(
            super::super::open_stream(&files, "vp9-vorbis.webm").unwrap(),
            LoadKind::Video {
                looped: false,
                clock: primary.clone(),
            },
        )
        .unwrap();
        let follower = Arc::new(PlaybackClock::new(now));
        prepared.sound.handle.add_video_clock(&follower);
        prepared.sound.handle.add_video_clock(&follower);
        let clocks = prepared.sound.handle.video_clocks.as_ref().unwrap().clone();
        assert_eq!(clocks.lock().unwrap().len(), 2);
        prepared.sound.on_start_processing();
        prepared
            .sound
            .process(&mut [], 0.0, &kira::info::MockInfoBuilder::new().build());
        let position = prepared.sound.handle.position();
        assert!((primary.elapsed(now + Duration::from_secs(10)) - position).abs() < 1e-9);
        assert!((follower.elapsed(now + Duration::from_secs(10)) - position).abs() < 1e-9);
        drop(follower);
        prepared
            .sound
            .process(&mut [], 0.0, &kira::info::MockInfoBuilder::new().build());
        assert_eq!(clocks.lock().unwrap().len(), 1);
    }

    #[test]
    fn dropping_an_unplayed_stream_stops_its_decoder_thread() {
        struct TestDecoder(Sender<()>);
        impl kira::sound::streaming::Decoder for TestDecoder {
            type Error = FromFileError;
            fn sample_rate(&self) -> u32 {
                48_000
            }
            fn num_frames(&self) -> usize {
                480_000
            }
            fn decode(&mut self) -> Result<Vec<kira::Frame>, Self::Error> {
                Ok(vec![kira::Frame::ZERO; 256])
            }
            fn seek(&mut self, index: usize) -> Result<usize, Self::Error> {
                Ok(index)
            }
        }
        impl Drop for TestDecoder {
            fn drop(&mut self) {
                let _ = self.0.send(());
            }
        }
        let (sender, receiver) = channel();
        let prepared = prepare_stream(
            StreamingSoundData::from_decoder(TestDecoder(sender)),
            LoadKind::Music {
                looped: true,
                volume: 1.0,
                fade_in: 0.0,
            },
        )
        .unwrap();
        drop(prepared);
        receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("cancelled stream must release its decoder");
    }
}
