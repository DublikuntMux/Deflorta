mod clock;
mod webm;
pub use clock::PlaybackClock;
mod planar;
pub use planar::{ColorMatrix, ColorRange, Plane, VideoFrame};

#[cfg(feature = "debug-formats")]
mod h264;

#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use log::{debug, error, info, warn};
use tokio::sync::mpsc::{Receiver, Sender, channel, error::TryRecvError};

use crate::files::GameFiles;
use crate::worker::runtime;

const QUEUE_AHEAD: usize = 4;

struct Frame {
    /// Presentation time in seconds since playback started.
    pts: f64,
    image: Arc<VideoFrame>,
}

enum Message {
    Frame(Frame),
    End(f64),
}

struct State {
    queue: VecDeque<Frame>,
    current: Option<(u64, Arc<VideoFrame>)>,
    decoding_done: bool,
    end_pts: f64,
    ended: bool,
    end_reported: bool,
}

pub struct VideoPlayer {
    src: String,
    looping: bool,
    clock: Arc<PlaybackClock>,
    size: Option<(u32, u32)>,
    receiver: RefCell<Receiver<Message>>,
    state: RefCell<State>,
}

impl VideoPlayer {
    #[must_use]
    ///
    /// # Panics
    ///
    /// Panics if the shared worker runtime cannot be started.
    pub fn open(src: &str, files: &GameFiles, path: &str, looping: bool, now: Instant) -> Self {
        let (sender, receiver) = channel(QUEUE_AHEAD);
        let size = probe_size(files, path)
            .map_err(|e| warn!("Cannot open video '{src}': {e:#}"))
            .ok();
        if let Some((w, h)) = size {
            info!("Playing video '{src}' ({w}x{h}, loop: {looping})");
        }
        let decode_files = files.clone();
        let decode_path = path.to_owned();
        let decode_src = src.to_owned();
        runtime().spawn_blocking(move || {
            let end_pts = match decode(&decode_files, &decode_path, looping, &sender) {
                Ok((frames, end_pts)) => {
                    debug!("Video '{decode_src}' decoding finished after {frames} frames");
                    end_pts
                }
                Err(err) => {
                    error!("Video '{decode_src}' failed: {err:#}");
                    0.0
                }
            };
            let _ = sender.blocking_send(Message::End(end_pts));
        });
        Self {
            src: src.to_owned(),
            looping,
            clock: Arc::new(PlaybackClock::new(now)),
            size,
            receiver: RefCell::new(receiver),
            state: RefCell::new(State {
                queue: VecDeque::new(),
                current: None,
                decoding_done: false,
                end_pts: 0.0,
                ended: false,
                end_reported: false,
            }),
        }
    }

    pub fn src(&self) -> &str {
        &self.src
    }

    pub const fn looping(&self) -> bool {
        self.looping
    }

    #[must_use]
    pub fn clock(&self) -> Arc<PlaybackClock> {
        self.clock.clone()
    }

    pub const fn size(&self) -> Option<(u32, u32)> {
        self.size
    }

    pub fn ended(&self) -> bool {
        self.state.borrow().ended
    }

    /// True once after playback reached the end.
    pub fn take_ended(&mut self) -> bool {
        let state = self.state.get_mut();
        let fire = state.ended && !state.end_reported;
        state.end_reported |= fire;
        if fire {
            info!("Video '{}' ended", self.src);
        }
        fire
    }

    /// The frame to show at `now`, with a serial that changes whenever the frame does.
    pub fn frame(&self, now: Instant) -> Option<(u64, Arc<VideoFrame>)> {
        let mut state = self.state.borrow_mut();
        let mut receiver = self.receiver.borrow_mut();
        let elapsed = self.clock.elapsed(now);
        loop {
            while state.queue.len() < QUEUE_AHEAD && !state.decoding_done {
                match receiver.try_recv() {
                    Ok(Message::Frame(frame)) => state.queue.push_back(frame),
                    Ok(Message::End(end_pts)) => {
                        state.end_pts = end_pts;
                        state.decoding_done = true;
                    }
                    Err(TryRecvError::Disconnected) => {
                        state.decoding_done = true;
                    }
                    Err(TryRecvError::Empty) => break,
                }
            }
            let Some(frame) = state.queue.pop_front_if(|frame| frame.pts <= elapsed) else {
                break;
            };
            let serial = state.current.as_ref().map_or(1, |(s, _)| s + 1);
            state.current = Some((serial, frame.image));
        }
        if state.decoding_done && state.queue.is_empty() && elapsed >= state.end_pts {
            state.ended = true;
        }
        state.current.clone()
    }
}

fn probe_size(files: &GameFiles, path: &str) -> Result<(u32, u32)> {
    #[cfg(feature = "debug-formats")]
    if !crate::media::is_webm(files, path)? {
        return h264::probe_size(files, path);
    }
    webm::probe_size(files, path)
}

fn decode(
    files: &GameFiles,
    path: &str,
    looping: bool,
    sender: &Sender<Message>,
) -> Result<(u64, f64)> {
    if sender.is_closed() {
        return Ok((0, 0.0));
    }
    #[cfg(feature = "debug-formats")]
    if !crate::media::is_webm(files, path)? {
        return h264::decode(files, path, looping, sender);
    }
    webm::decode(files, path, looping, sender)
}
