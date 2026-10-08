mod h264;
mod webm;

#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, sync_channel};
use std::time::Instant;

use anyhow::Result;
use log::{debug, error, info, warn};

use crate::files::GameFiles;

const QUEUE_AHEAD: usize = 4;

struct Frame {
    /// Presentation time in seconds since playback started.
    pts: f64,
    image: Arc<image::RgbaImage>,
}

enum Message {
    Frame(Frame),
    End(f64),
}

struct State {
    queue: VecDeque<Frame>,
    current: Option<(u64, Arc<image::RgbaImage>)>,
    decoding_done: bool,
    end_pts: f64,
    ended: bool,
    end_reported: bool,
}

pub struct VideoPlayer {
    src: String,
    looping: bool,
    start: Instant,
    size: Option<(u32, u32)>,
    receiver: Receiver<Message>,
    state: RefCell<State>,
}

impl VideoPlayer {
    pub fn open(src: &str, files: &GameFiles, path: &str, looping: bool, now: Instant) -> Self {
        let (sender, receiver) = sync_channel(QUEUE_AHEAD);
        let size = probe_size(files, path)
            .map_err(|e| warn!("Cannot open video '{src}': {e:#}"))
            .ok();
        if let Some((w, h)) = size {
            info!("Playing video '{src}' ({w}x{h}, loop: {looping})");
        }
        let thread_files = files.clone();
        let thread_path = path.to_owned();
        let thread_src = src.to_owned();
        std::thread::Builder::new()
            .name(format!("video {src}"))
            .spawn(move || {
                let end_pts = match decode(&thread_files, &thread_path, looping, &sender) {
                    Ok((frames, end_pts)) => {
                        debug!("Video '{thread_src}' decoding finished after {frames} frames");
                        end_pts
                    }
                    Err(err) => {
                        error!("Video '{thread_src}' failed: {err:#}");
                        0.0
                    }
                };
                let _ = sender.send(Message::End(end_pts));
            })
            .expect("spawn video thread");
        Self {
            src: src.to_owned(),
            looping,
            start: now,
            size,
            receiver,
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

    pub const fn started(&self) -> Instant {
        self.start
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
    pub fn frame(&self, now: Instant) -> Option<(u64, Arc<image::RgbaImage>)> {
        let mut state = self.state.borrow_mut();
        let elapsed = now.saturating_duration_since(self.start).as_secs_f64();
        loop {
            while state.queue.len() < QUEUE_AHEAD && !state.decoding_done {
                match self.receiver.try_recv() {
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
            match state.queue.front() {
                Some(frame) if frame.pts <= elapsed => {
                    let frame = state.queue.pop_front().unwrap();
                    let serial = state.current.as_ref().map_or(1, |(s, _)| s + 1);
                    state.current = Some((serial, frame.image));
                }
                _ => break,
            }
        }
        if state.decoding_done && state.queue.is_empty() && elapsed >= state.end_pts {
            state.ended = true;
        }
        state.current.clone()
    }
}

fn probe_size(files: &GameFiles, path: &str) -> Result<(u32, u32)> {
    if is_webm(path) {
        return webm::probe_size(files, path);
    }
    h264::probe_size(files, path)
}

fn decode(
    files: &GameFiles,
    path: &str,
    looping: bool,
    sender: &SyncSender<Message>,
) -> Result<(u64, f64)> {
    if is_webm(path) {
        webm::decode(files, path, looping, sender)
    } else {
        h264::decode(files, path, looping, sender)
    }
}

pub fn is_webm(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("webm"))
}
