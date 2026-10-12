use std::sync::Arc;
use std::sync::mpsc::SyncSender;

use super::{ColorMatrix, ColorRange, Frame, Message, Plane, VideoFrame};
use crate::files::GameFiles;
use anyhow::{Context, Result, ensure};
use deflorta_vpx::{ColorSpace, Decoder as Vp9Decoder};
use log::trace;
use oxideav_core::{Demuxer, Error, NullCodecResolver, StreamInfo};
use oxideav_mkv::demux::{AlphaMode, MkvDemuxer, open_typed};

#[cfg(feature = "debug-formats")]
use oxideav_vp8::state::Vp8DecoderState;

fn open(files: &GameFiles, path: &str) -> Result<MkvDemuxer> {
    // Symphonia discards BlockAdditional, which carries WebM's alpha bitstream.
    Ok(open_typed(
        Box::new(files.open_file(path)?),
        &NullCodecResolver,
    )?)
}

fn video_track(reader: &MkvDemuxer) -> Result<&StreamInfo> {
    reader
        .streams()
        .iter()
        .find(|track| {
            let codec = track.params.codec_id.as_str();
            codec == "vp9" || (cfg!(feature = "debug-formats") && codec == "vp8")
        })
        .context("no supported WebM video track (expected VP9, or VP8 in debug builds)")
}

pub(super) fn probe_size(files: &GameFiles, path: &str) -> Result<(u32, u32)> {
    let reader = open(files, path)?;
    let params = &video_track(&reader)?.params;
    Ok((
        params.width.context("WebM video width is missing")?,
        params.height.context("WebM video height is missing")?,
    ))
}

fn decoder_threads() -> u32 {
    std::thread::available_parallelism().map_or(1, |threads| {
        u32::try_from(threads.get()).unwrap_or(u32::MAX)
    })
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
        let mut reader = open(files, path)?;
        let track = video_track(&reader)?;
        let track_id = track.index;
        let vp9 = track.params.codec_id.as_str() == "vp9";
        let has_alpha = reader.video_alpha_mode(track_id) == Some(AlphaMode::Present);
        #[cfg(feature = "debug-formats")]
        let mut vp8_decoder = Vp8DecoderState::new();
        #[cfg(feature = "debug-formats")]
        let mut vp8_alpha_decoder = Vp8DecoderState::new();
        let mut vp9_decoder = vp9
            .then(|| Vp9Decoder::new(decoder_threads()))
            .transpose()?;
        let mut vp9_alpha_decoder = (vp9 && has_alpha)
            .then(|| Vp9Decoder::new(decoder_threads()))
            .transpose()?;
        let mut end = reader.duration_micros().map_or(0.0, |duration| {
            oxideav_core::TimeBase::MICROS.seconds_of(duration)
        });
        let mut last_pts: Option<f64> = None;
        let mut last_interval = 0.0;
        let first_frame = frames;
        loop {
            let packet = match reader.next_packet() {
                Ok(packet) => packet,
                Err(Error::Eof) => break,
                Err(error) => return Err(error.into()),
            };
            if packet.stream_index != track_id {
                continue;
            }
            let pts = packet
                .time_base
                .seconds_of(packet.pts.context("WebM timestamp is missing")?);
            let duration = packet.time_base.seconds_of(packet.duration.unwrap_or(0));
            if let Some(previous) = last_pts {
                last_interval = (pts - previous).max(0.0);
            }
            last_pts = Some(pts);
            end = end.max(pts + duration);
            let mut image = if let Some(decoder) = &mut vp9_decoder {
                vp9_image(decoder, &packet.data)?
            } else {
                #[cfg(feature = "debug-formats")]
                {
                    vp8_image(&mut vp8_decoder, &packet.data)?
                }
                #[cfg(not(feature = "debug-formats"))]
                {
                    unreachable!("VP8 tracks are excluded from release playback")
                }
            };
            if has_alpha
                && let Some(addition) = reader
                    .block_additions()
                    .iter()
                    .find(|a| a.block_add_id() == 1)
            {
                let shown = if let Some(decoder) = &mut vp9_alpha_decoder {
                    vp9_alpha(decoder, addition.data(), image.as_mut())?
                } else {
                    #[cfg(feature = "debug-formats")]
                    {
                        vp8_alpha(&mut vp8_alpha_decoder, addition.data(), image.as_mut())?
                    }
                    #[cfg(not(feature = "debug-formats"))]
                    {
                        unreachable!("VP8 tracks are excluded from release playback")
                    }
                };
                ensure!(
                    shown == image.is_some(),
                    "WebM color and alpha frame counts differ"
                );
            }
            if let Some(image) = image {
                if sender
                    .send(Message::Frame(Frame {
                        pts: pts + offset,
                        image: Arc::new(image),
                    }))
                    .is_err()
                {
                    return Ok((frames, offset + end));
                }
                frames += 1;
            }
        }
        // Streaming WebM may omit Duration and the last packet's duration. Use the
        // observed frame interval so the last picture stays on screen for a frame.
        if let Some(pts) = last_pts {
            end = end.max(pts + last_interval);
        }
        if !looping || end <= 0.0 || frames == first_frame {
            return Ok((frames, offset + end));
        }
        trace!("Looping video {path}");
        offset += end;
    }
}

fn vp9_image(decoder: &mut Vp9Decoder, data: &[u8]) -> Result<Option<VideoFrame>> {
    // libvpx splits superframes and returns only the shown picture.
    let Some(frame) = decoder.decode(data)? else {
        return Ok(None);
    };
    let range = if frame.full_range() {
        ColorRange::Full
    } else {
        ColorRange::Limited
    };
    let matrix = match frame.color_space() {
        ColorSpace::Srgb => ColorMatrix::Identity,
        ColorSpace::Bt709 => ColorMatrix::Bt709,
        ColorSpace::Bt2020 => ColorMatrix::Bt2020,
        ColorSpace::Smpte240 => ColorMatrix::Smpte240,
        ColorSpace::Unknown | ColorSpace::Bt601 | ColorSpace::Smpte170 => ColorMatrix::Bt601,
    };
    let plane = |index| (frame.plane(index), frame.stride(index));
    VideoFrame::copy(
        (frame.width(), frame.height()),
        [plane(0), plane(1), plane(2)],
        frame.subsampling(),
        range,
        matrix,
    )
    .map(Some)
}

fn vp9_alpha(
    decoder: &mut Vp9Decoder,
    data: &[u8],
    image: Option<&mut VideoFrame>,
) -> Result<bool> {
    let Some(frame) = decoder.decode(data)? else {
        return Ok(false);
    };
    if let Some(image) = image {
        apply_alpha(
            image,
            (frame.width(), frame.height()),
            (frame.plane(0), frame.stride(0)),
        )?;
    }
    Ok(true)
}

#[cfg(feature = "debug-formats")]
fn vp8_image(decoder: &mut Vp8DecoderState, data: &[u8]) -> Result<Option<VideoFrame>> {
    let frame = decoder.decode_frame(data)?;
    if decoder.last_frame_shown() != Some(true) {
        return Ok(None);
    }
    let chroma_stride = frame.width.div_ceil(2);
    VideoFrame::copy(
        (frame.width, frame.height),
        [
            (&frame.y, frame.width),
            (&frame.u, chroma_stride),
            (&frame.v, chroma_stride),
        ],
        (true, true),
        ColorRange::Limited,
        ColorMatrix::Bt601,
    )
    .map(Some)
}

#[cfg(feature = "debug-formats")]
fn vp8_alpha(
    decoder: &mut Vp8DecoderState,
    data: &[u8],
    image: Option<&mut VideoFrame>,
) -> Result<bool> {
    let frame = decoder.decode_frame(data)?;
    if decoder.last_frame_shown() != Some(true) {
        return Ok(false);
    }
    if let Some(image) = image {
        apply_alpha(image, (frame.width, frame.height), (&frame.y, frame.width))?;
    }
    Ok(true)
}

fn apply_alpha(
    image: &mut VideoFrame,
    size: (u32, u32),
    (alpha, stride): (&[u8], u32),
) -> Result<()> {
    ensure!(
        image.dimensions() == size,
        "WebM color and alpha dimensions differ"
    );
    image.alpha = Some(Plane::copy(alpha, stride, size.0, size.1)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_is_full_range_and_preserves_straight_rgb() {
        let mut image = VideoFrame::copy(
            (5, 2),
            [(&[120; 10], 5), (&[180; 10], 5), (&[60; 10], 5)],
            (false, false),
            ColorRange::Full,
            ColorMatrix::Identity,
        )
        .unwrap();
        // A padded stride must skip the bytes past each visible row.
        let alpha = [0, 16, 128, 235, 255, 99, 99, 1, 2, 3, 4, 5];
        apply_alpha(&mut image, (5, 2), (&alpha, 7)).unwrap();
        let expected = [0, 16, 128, 235, 255, 1, 2, 3, 4, 5];
        for (pixel, sample) in super::super::tests::reference_rgba(&image)
            .pixels()
            .zip(expected)
        {
            assert_eq!(pixel.0, [60, 120, 180, sample]);
        }
        assert!(apply_alpha(&mut image, (4, 2), (&alpha, 7)).is_err());
    }
}
