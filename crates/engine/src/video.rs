//! Video playback: H.264 in MP4 and VP8/VP9 in `WebM`, decoded on a background
//! thread. `WebM` demuxing and decoding use pure Rust libraries.
//! Frames are timed against the wall clock;
//! the engine plays the file's audio track separately.

mod webm;

#[cfg(test)]
mod tests;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::BufReader;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, sync_channel};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use log::{debug, error, info, trace, warn};
use num_traits::AsPrimitive;
use openh264::decoder::Decoder;
use openh264::formats::YUVSource;

use crate::files::{GameFiles, GameReader};

/// Decoded frames buffered ahead of playback.
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

type Mp4Reader = mp4::Mp4Reader<BufReader<GameReader>>;

fn open_reader(files: &GameFiles, path: &str) -> Result<Mp4Reader> {
    let file = files.open_file(path)?;
    let size = file.size()?;
    Ok(mp4::Mp4Reader::read_header(file.buffered(), size)?)
}

fn video_track(reader: &Mp4Reader) -> Result<&mp4::Mp4Track> {
    reader
        .tracks()
        .values()
        .find(|t| matches!(t.media_type(), Ok(mp4::MediaType::H264)))
        .context("no H.264 video track")
}

fn probe_size(files: &GameFiles, path: &str) -> Result<(u32, u32)> {
    if is_webm(path) {
        return webm::probe_size(files, path);
    }
    let reader = open_reader(files, path)?;
    let track = video_track(&reader)?;
    Ok((u32::from(track.width()), u32::from(track.height())))
}

/// Converts length-prefixed NAL units (AVCC) to Annex B start codes.
fn to_annex_b(sample: &[u8], out: &mut Vec<u8>) {
    let mut rest = sample;
    while rest.len() >= 4 {
        let Ok(len) = usize::try_from(u32::from_be_bytes([rest[0], rest[1], rest[2], rest[3]]))
        else {
            break;
        };
        rest = &rest[4..];
        if len > rest.len() {
            break;
        }
        out.extend_from_slice(&[0, 0, 0, 1]);
        out.extend_from_slice(&rest[..len]);
        rest = &rest[len..];
    }
}

/// Decodes frames until the end (or until the player is dropped). Returns the frame count.
fn decode(
    files: &GameFiles,
    path: &str,
    looping: bool,
    sender: &SyncSender<Message>,
) -> Result<(u64, f64)> {
    if is_webm(path) {
        webm::decode(files, path, looping, sender)
    } else {
        decode_mp4(files, path, looping, sender)
    }
}

pub fn is_webm(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("webm"))
}

fn decode_mp4(
    files: &GameFiles,
    path: &str,
    looping: bool,
    sender: &SyncSender<Message>,
) -> Result<(u64, f64)> {
    let mut offset = 0.0;
    let mut frames = 0;
    loop {
        let mut reader = open_reader(files, path)?;
        let track = video_track(&reader)?;
        let track_id = track.track_id();
        let timescale = f64::from(track.timescale());
        let duration = track.duration().as_secs_f64();
        let mut header = Vec::new();
        for nal in [
            track.sequence_parameter_set()?,
            track.picture_parameter_set()?,
        ] {
            header.extend_from_slice(&[0, 0, 0, 1]);
            header.extend_from_slice(nal);
        }
        let count = reader.sample_count(track_id)?;
        let mut decoder = Decoder::new()?;
        let mut packet = Vec::new();
        for id in 1..=count {
            let Some(sample) = reader.read_sample(track_id, id)? else {
                continue;
            };
            packet.clear();
            if id == 1 {
                packet.extend_from_slice(&header);
            }
            to_annex_b(&sample.bytes, &mut packet);
            let start_time: f64 = sample.start_time.as_();
            let pts = (start_time + f64::from(sample.rendering_offset)) / timescale + offset;
            let Some(yuv) = decoder.decode(&packet)? else {
                continue;
            };
            let (w, h) = yuv.dimensions();
            let width = u32::try_from(w).context("video frame width exceeds u32")?;
            let height = u32::try_from(h).context("video frame height exceeds u32")?;
            let mut rgba = vec![0; w * h * 4];
            yuv.write_rgba8(&mut rgba);
            let Some(image) = image::RgbaImage::from_raw(width, height, rgba) else {
                bail!("frame size mismatch");
            };
            if sender
                .send(Message::Frame(Frame {
                    pts,
                    image: Arc::new(image),
                }))
                .is_err()
            {
                // The player was dropped (the video left the screen).
                return Ok((frames, offset + duration));
            }
            frames += 1;
        }
        if !looping || duration <= 0.0 {
            return Ok((frames, offset + duration));
        }
        trace!("Looping video {path}");
        offset += duration;
    }
}
