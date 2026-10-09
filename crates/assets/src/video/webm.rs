use std::sync::Arc;
use std::sync::mpsc::SyncSender;

use anyhow::{Context, Result, bail, ensure};
use deflorta_vpx::{ColorSpace, Decoder as Vp9Decoder};
use log::trace;
use oxideav_core::{Demuxer, Error, NullCodecResolver, StreamInfo};
use oxideav_mkv::demux::{AlphaMode, MkvDemuxer, open_typed};
use yuv::{YuvPlanarImage, YuvRange, YuvStandardMatrix};

use super::{Frame, Message};
use crate::files::GameFiles;

#[cfg(feature = "debug-formats")]
use oxideav_vp8::state::Vp8DecoderState;

/// One 8-bit picture plane and its row stride in bytes.
type Plane<'a> = (&'a [u8], u32);

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

fn vp9_image(decoder: &mut Vp9Decoder, data: &[u8]) -> Result<Option<image::RgbaImage>> {
    // libvpx splits superframes and returns only the shown picture.
    let Some(frame) = decoder.decode(data)? else {
        return Ok(None);
    };
    let range = if frame.full_range() {
        YuvRange::Full
    } else {
        YuvRange::Limited
    };
    let matrix = match frame.color_space() {
        ColorSpace::Srgb => None,
        ColorSpace::Bt709 => Some(YuvStandardMatrix::Bt709),
        ColorSpace::Bt2020 => Some(YuvStandardMatrix::Bt2020),
        ColorSpace::Smpte240 => Some(YuvStandardMatrix::Smpte240),
        ColorSpace::Unknown | ColorSpace::Bt601 | ColorSpace::Smpte170 => {
            Some(YuvStandardMatrix::Bt601)
        }
    };
    let plane = |index| (frame.plane(index), frame.stride(index));
    to_rgba(
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
    image: Option<&mut image::RgbaImage>,
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
fn vp8_image(decoder: &mut Vp8DecoderState, data: &[u8]) -> Result<Option<image::RgbaImage>> {
    let frame = decoder.decode_frame(data)?;
    if decoder.last_frame_shown() != Some(true) {
        return Ok(None);
    }
    let chroma_stride = frame.width.div_ceil(2);
    to_rgba(
        (frame.width, frame.height),
        [
            (&frame.y, frame.width),
            (&frame.u, chroma_stride),
            (&frame.v, chroma_stride),
        ],
        (true, true),
        YuvRange::Limited,
        Some(YuvStandardMatrix::Bt601),
    )
    .map(Some)
}

#[cfg(feature = "debug-formats")]
fn vp8_alpha(
    decoder: &mut Vp8DecoderState,
    data: &[u8],
    image: Option<&mut image::RgbaImage>,
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
    image: &mut image::RgbaImage,
    size: (u32, u32),
    (alpha, stride): Plane<'_>,
) -> Result<()> {
    ensure!(
        image.dimensions() == size,
        "WebM color and alpha dimensions differ"
    );
    // Alpha is the raw luma plane, not limited-range YUV color. Keep RGB straight;
    // the renderer premultiplies it when blending the texture.
    let width = size.0 as usize;
    for (pixels, alpha) in image
        .as_mut()
        .chunks_exact_mut(width * 4)
        .zip(alpha.chunks(stride as usize))
    {
        for (pixel, alpha) in pixels
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(&alpha[..width])
        {
            pixel[3] = *alpha;
        }
    }
    Ok(())
}

fn to_rgba(
    (width, height): (u32, u32),
    [luma, cb, cr]: [Plane<'_>; 3],
    subsampling: (bool, bool),
    range: YuvRange,
    // `None` selects VP9's RGB mode, which stores G, B, R in the Y, U, V planes.
    matrix: Option<YuvStandardMatrix>,
) -> Result<image::RgbaImage> {
    let mut image = image::RgbaImage::new(width, height);
    let Some(matrix) = matrix else {
        fn row((plane, stride): Plane<'_>, row: usize) -> &[u8] {
            &plane[row * stride as usize..]
        }
        for (index, pixels) in image
            .as_mut()
            .chunks_exact_mut(width as usize * 4)
            .enumerate()
        {
            let (green, blue, red) = (row(luma, index), row(cb, index), row(cr, index));
            for (column, pixel) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                *pixel = [red[column], green[column], blue[column], 255];
            }
        }
        return Ok(image);
    };
    let planar = YuvPlanarImage {
        y_plane: luma.0,
        y_stride: luma.1,
        u_plane: cb.0,
        u_stride: cb.1,
        v_plane: cr.0,
        v_stride: cr.1,
        width,
        height,
    };
    let convert = match subsampling {
        (true, true) => yuv::yuv420_to_rgba,
        (true, false) => yuv::yuv422_to_rgba,
        (false, false) => yuv::yuv444_to_rgba,
        (false, true) => bail!("VP9 4:4:0 chroma subsampling is unsupported"),
    };
    convert(
        &planar,
        image.as_mut(),
        width.checked_mul(4).context("video stride overflow")?,
        range,
        matrix,
    )?;
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alpha_is_full_range_and_preserves_straight_rgb() {
        let mut image = image::RgbaImage::from_pixel(5, 2, image::Rgba([60, 120, 180, 255]));
        // A padded stride must skip the bytes past each visible row.
        let alpha = [0, 16, 128, 235, 255, 99, 99, 1, 2, 3, 4, 5];
        apply_alpha(&mut image, (5, 2), (&alpha, 7)).unwrap();
        let expected = [0, 16, 128, 235, 255, 1, 2, 3, 4, 5];
        for (pixel, sample) in image.pixels().zip(expected) {
            assert_eq!(pixel.0, [60, 120, 180, sample]);
        }
        assert!(apply_alpha(&mut image, (4, 2), (&alpha, 7)).is_err());
    }
}
