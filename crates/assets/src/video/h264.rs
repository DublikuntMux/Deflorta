use std::io::BufReader;
use std::sync::Arc;
use std::sync::mpsc::SyncSender;

use anyhow::{Context, Result, bail, ensure};
use mp4::WriteBox;
use oxideav_core::{CodecId, Decoder, Error, Packet, TimeBase, VideoFrame};
use oxideav_h264::h264_decoder::H264CodecDecoder;
use oxideav_h264::sps::Sps;
use yuv::{YuvPlanarImage, YuvRange, YuvStandardMatrix};

use super::{Frame, Message};
use crate::files::{GameFiles, GameReader};

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
        .find(|track| matches!(track.media_type(), Ok(mp4::MediaType::H264)))
        .context("no H.264 video track")
}

pub(super) fn probe_size(files: &GameFiles, path: &str) -> Result<(u32, u32)> {
    let reader = open_reader(files, path)?;
    let track = video_track(&reader)?;
    Ok((u32::from(track.width()), u32::from(track.height())))
}

pub(super) fn decode(
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
        ensure!(track.timescale() > 0, "MP4 video timescale is zero");
        let time_base = TimeBase::from_rate(track.timescale());
        let duration = track.duration().as_secs_f64();
        let timeline_offset = presentation_offset(&reader, track)?;
        let avcc = &track
            .trak
            .mdia
            .minf
            .stbl
            .stsd
            .avc1
            .as_ref()
            .context("missing H.264 sample description")?
            .avcc;
        let mut extra = Vec::new();
        avcc.write_box(&mut extra)?;
        let mut decoder = H264CodecDecoder::new(CodecId::new("h264"));
        // WriteBox includes the eight-byte MP4 box header; the codec needs its payload.
        decoder.consume_extradata(&extra[8..])?;
        let count = reader.sample_count(track_id)?;
        let previous_frames = frames;
        for id in 1..=count {
            let sample = reader
                .read_sample(track_id, id)?
                .context("missing MP4 video sample")?;
            let dts = i64::try_from(sample.start_time).context("MP4 timestamp overflow")?;
            let pts = dts
                .checked_add(i64::from(sample.rendering_offset))
                .context("MP4 presentation timestamp overflow")?;
            let packet = Packet::new(track_id, time_base, sample.bytes.to_vec())
                .with_dts(dts)
                .with_pts(pts);
            decoder
                .send_packet(&packet)
                .with_context(|| format!("decoding MP4 sample {id}"))?;
            if !drain(
                &mut decoder,
                time_base,
                offset + timeline_offset,
                sender,
                &mut frames,
            )? {
                return Ok((frames, offset + duration));
            }
        }
        // Finalize the last picture and drain B-frames still held for reordering.
        decoder.flush()?;
        if !drain(
            &mut decoder,
            time_base,
            offset + timeline_offset,
            sender,
            &mut frames,
        )? || !looping
            || duration <= 0.0
            || frames == previous_frames
        {
            return Ok((frames, offset + duration));
        }
        log::trace!("Looping video {path}");
        offset += duration;
    }
}

/// Map media timestamps onto the movie timeline. Encoders commonly use an edit
/// to remove the initial decode delay of B-frames.
fn presentation_offset(reader: &Mp4Reader, track: &mp4::Mp4Track) -> Result<f64> {
    let mut offset = 0.0;
    if let Some(edits) = track.trak.edts.as_ref().and_then(|edts| edts.elst.as_ref()) {
        ensure!(reader.timescale() > 0, "MP4 movie timescale is zero");
        for edit in &edits.entries {
            ensure!(
                edit.media_rate == 1 && edit.media_rate_fraction == 0,
                "MP4 video playback requires normal-rate edits"
            );
            // The mp4 crate stores the signed -1 (empty edit) as an unsigned value.
            let empty = if edits.version == 1 {
                u64::MAX
            } else {
                u64::from(u32::MAX)
            };
            if edit.media_time == empty {
                offset += TimeBase::from_rate(reader.timescale()).seconds_of(
                    i64::try_from(edit.segment_duration).context("MP4 edit duration overflow")?,
                );
            } else {
                offset -= TimeBase::from_rate(track.timescale()).seconds_of(
                    i64::try_from(edit.media_time).context("MP4 edit timestamp overflow")?,
                );
                break;
            }
        }
    }
    Ok(offset)
}

/// Returns false when the player has been dropped.
fn drain(
    decoder: &mut H264CodecDecoder,
    time_base: TimeBase,
    offset: f64,
    sender: &SyncSender<Message>,
    frames: &mut u64,
) -> Result<bool> {
    ensure!(
        decoder.decode_error_count() == 0,
        "H.264 slice decoding failed"
    );
    loop {
        let frame = match decoder.receive_frame() {
            Ok(oxideav_core::Frame::Video(frame)) => frame,
            Ok(_) => bail!("H.264 decoder returned a non-video frame"),
            Err(Error::NeedMore | Error::Eof) => return Ok(true),
            Err(error) => return Err(error.into()),
        };
        let pts = time_base.seconds_of(frame.pts.context("H.264 frame has no timestamp")?) + offset;
        let sps = decoder
            .active_sps()
            .context("missing H.264 sequence parameters")?;
        let image = to_rgba(&frame, sps)?;
        if sender
            .send(Message::Frame(Frame {
                pts,
                image: Arc::new(image),
            }))
            .is_err()
        {
            return Ok(false);
        }
        *frames += 1;
    }
}

fn to_rgba(frame: &VideoFrame, sps: &Sps) -> Result<image::RgbaImage> {
    ensure!(
        sps.chroma_array_type() == 1
            && sps.bit_depth_luma_minus8 == 0
            && sps.bit_depth_chroma_minus8 == 0,
        "H.264 playback requires 8-bit 4:2:0 video"
    );
    let [y, u, v] = frame.planes.as_slice() else {
        bail!("H.264 frame must contain three image planes");
    };
    let width = sps
        .pic_width_in_mbs()
        .checked_mul(16)
        .context("H.264 width overflow")?;
    let height = sps
        .frame_height_in_mbs()
        .checked_mul(16)
        .context("H.264 height overflow")?;
    let planar = YuvPlanarImage {
        y_plane: &y.data,
        y_stride: u32::try_from(y.stride)?,
        u_plane: &u.data,
        u_stride: u32::try_from(u.stride)?,
        v_plane: &v.data,
        v_stride: u32::try_from(v.stride)?,
        width,
        height,
    };
    let signal = sps
        .vui
        .as_ref()
        .and_then(|vui| vui.video_signal_type.as_ref());
    let range = if signal.is_some_and(|signal| signal.video_full_range_flag) {
        YuvRange::Full
    } else {
        YuvRange::Limited
    };
    let matrix = match signal
        .and_then(|signal| signal.colour_description.as_ref())
        .map(|color| color.matrix_coefficients)
    {
        Some(1) => YuvStandardMatrix::Bt709,
        Some(7) => YuvStandardMatrix::Smpte240,
        Some(9) => YuvStandardMatrix::Bt2020,
        _ => YuvStandardMatrix::Bt601,
    };
    let mut image = image::RgbaImage::new(width, height);
    yuv::yuv420_to_rgba(
        &planar,
        image.as_mut(),
        width.checked_mul(4).context("video stride overflow")?,
        range,
        matrix,
    )?;
    if let Some(crop) = &sps.frame_cropping {
        let crop_y = if sps.frame_mbs_only_flag { 2 } else { 4 };
        let left = crop.left.checked_mul(2).context("H.264 crop overflow")?;
        let right = crop.right.checked_mul(2).context("H.264 crop overflow")?;
        let top = crop
            .top
            .checked_mul(crop_y)
            .context("H.264 crop overflow")?;
        let bottom = crop
            .bottom
            .checked_mul(crop_y)
            .context("H.264 crop overflow")?;
        let visible_width = width
            .checked_sub(left)
            .and_then(|w| w.checked_sub(right))
            .filter(|w| *w > 0)
            .context("invalid H.264 horizontal crop")?;
        let visible_height = height
            .checked_sub(top)
            .and_then(|h| h.checked_sub(bottom))
            .filter(|h| *h > 0)
            .context("invalid H.264 vertical crop")?;
        image =
            image::imageops::crop_imm(&image, left, top, visible_width, visible_height).to_image();
    }
    Ok(image)
}
