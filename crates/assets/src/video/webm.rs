use std::sync::Arc;
use std::sync::mpsc::SyncSender;

use anyhow::{Context, Result, bail, ensure};
use log::trace;
use oxideav_core::{Demuxer, Error, NullCodecResolver, StreamInfo};
use oxideav_mkv::demux::{AlphaMode, MkvDemuxer, open_typed};
use oxideav_vp9::{
    ColorConfig, ColorSpace, FrameType, Vp9DecodedFrame, Vp9SequenceDecoder,
    parse_uncompressed_header,
};
use yuv::{YuvPlanarImage, YuvRange, YuvStandardMatrix};

use super::{Frame, Message};
use crate::files::GameFiles;

#[cfg(feature = "debug-formats")]
use oxideav_vp8::state::Vp8DecoderState;

const DEFAULT_COLOR: ColorConfig = ColorConfig {
    bit_depth: 8,
    color_space: ColorSpace::Bt601,
    color_range_full: false,
    subsampling_x: true,
    subsampling_y: true,
};

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
        let mut vp9_decoder = Vp9SequenceDecoder::new();
        let mut vp9_alpha_decoder = Vp9SequenceDecoder::new();
        let mut color = DEFAULT_COLOR;
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
            let mut images = if vp9 {
                vp9_images(&mut vp9_decoder, &mut color, &packet.data)?
            } else {
                #[cfg(feature = "debug-formats")]
                {
                    vp8_images(&mut vp8_decoder, &packet.data, color)?
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
                let alpha = if vp9 {
                    vp9_alpha(&mut vp9_alpha_decoder, addition.data())?
                } else {
                    #[cfg(feature = "debug-formats")]
                    {
                        vp8_alpha(&mut vp8_alpha_decoder, addition.data())?
                    }
                    #[cfg(not(feature = "debug-formats"))]
                    {
                        unreachable!("VP8 tracks are excluded from release playback")
                    }
                };
                ensure!(
                    images.len() == alpha.len(),
                    "WebM color and alpha frame counts differ"
                );
                for (image, alpha) in images.iter_mut().zip(&alpha) {
                    apply_alpha(image, alpha)?;
                }
            }
            for image in images {
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

fn vp9_images(
    decoder: &mut Vp9SequenceDecoder,
    color: &mut ColorConfig,
    data: &[u8],
) -> Result<Vec<image::RgbaImage>> {
    let mut images = Vec::new();
    // A WebM packet may contain a hidden reference frame followed by a shown frame.
    for bytes in oxideav_vp9::split_superframe(data) {
        // Inter frames inherit the last intra frame's color configuration.
        if let Ok(header) = parse_uncompressed_header(bytes)
            && !header.show_existing_frame
            && (header.frame_type == FrameType::KeyFrame || header.intra_only)
        {
            *color = header.color_config;
        }
        if let Some(decoded) = decoder.push_frame(bytes)? {
            images.push(vp9_rgba(&decoded, *color)?);
        }
    }
    Ok(images)
}

#[cfg(feature = "debug-formats")]
fn vp8_images(
    decoder: &mut Vp8DecoderState,
    data: &[u8],
    color: ColorConfig,
) -> Result<Vec<image::RgbaImage>> {
    let frame = decoder.decode_frame(data)?;
    if decoder.last_frame_shown() != Some(true) {
        return Ok(Vec::new());
    }
    Ok(vec![to_rgba(
        frame.width,
        frame.height,
        [&frame.y, &frame.u, &frame.v],
        (true, true),
        color,
    )?])
}

#[cfg(feature = "debug-formats")]
fn vp8_alpha(decoder: &mut Vp8DecoderState, data: &[u8]) -> Result<Vec<image::GrayImage>> {
    let frame = decoder.decode_frame(data)?;
    if decoder.last_frame_shown() != Some(true) {
        return Ok(Vec::new());
    }
    Ok(vec![
        image::GrayImage::from_raw(frame.width, frame.height, frame.y)
            .context("invalid VP8 alpha plane size")?,
    ])
}

fn vp9_alpha(decoder: &mut Vp9SequenceDecoder, data: &[u8]) -> Result<Vec<image::GrayImage>> {
    let mut images = Vec::new();
    for bytes in oxideav_vp9::split_superframe(data) {
        if let Some(decoded) = decoder.push_frame(bytes)? {
            let shift = decoded
                .bit_depth
                .checked_sub(8)
                .context("unsupported VP9 alpha bit depth")?;
            let samples = decoded
                .y
                .iter()
                .map(|sample| u8::try_from(sample >> shift))
                .collect::<std::result::Result<Vec<_>, _>>()?;
            images.push(
                image::GrayImage::from_raw(decoded.width, decoded.height, samples)
                    .context("invalid VP9 alpha plane size")?,
            );
        }
    }
    Ok(images)
}

fn apply_alpha(image: &mut image::RgbaImage, alpha: &image::GrayImage) -> Result<()> {
    ensure!(
        image.dimensions() == alpha.dimensions(),
        "WebM color and alpha dimensions differ"
    );
    // Alpha is the raw luma plane, not limited-range YUV color. Keep RGB straight;
    // the renderer premultiplies it when blending the texture.
    for (pixel, alpha) in image.pixels_mut().zip(alpha.pixels()) {
        pixel[3] = alpha[0];
    }
    Ok(())
}

fn vp9_rgba(decoded: &Vp9DecodedFrame, color: ColorConfig) -> Result<image::RgbaImage> {
    let shift = decoded
        .bit_depth
        .checked_sub(8)
        .context("unsupported VP9 bit depth")?;
    let planes: Vec<Vec<u8>> = [&decoded.y, &decoded.u, &decoded.v]
        .iter()
        .map(|plane| {
            plane
                .iter()
                .map(|sample| u8::try_from(sample >> shift))
                .collect()
        })
        .collect::<std::result::Result<_, _>>()?;
    to_rgba(
        decoded.width,
        decoded.height,
        [&planes[0], &planes[1], &planes[2]],
        (decoded.subsampling_x, decoded.subsampling_y),
        color,
    )
}

fn to_rgba(
    width: u32,
    height: u32,
    planes: [&[u8]; 3],
    subsampling: (bool, bool),
    color: ColorConfig,
) -> Result<image::RgbaImage> {
    let mut image = image::RgbaImage::new(width, height);
    if color.color_space == ColorSpace::Rgb {
        // VP9's RGB mode stores G, B, R in the Y, U, V planes.
        for (index, pixel) in image.pixels_mut().enumerate() {
            *pixel = image::Rgba([planes[2][index], planes[0][index], planes[1][index], 255]);
        }
        return Ok(image);
    }
    let chroma_width = if subsampling.0 {
        width.div_ceil(2)
    } else {
        width
    };
    let planar = YuvPlanarImage {
        y_plane: planes[0],
        y_stride: width,
        u_plane: planes[1],
        u_stride: chroma_width,
        v_plane: planes[2],
        v_stride: chroma_width,
        width,
        height,
    };
    let range = if color.color_range_full {
        YuvRange::Full
    } else {
        YuvRange::Limited
    };
    let matrix = match color.color_space {
        ColorSpace::Bt709 => YuvStandardMatrix::Bt709,
        ColorSpace::Bt2020 => YuvStandardMatrix::Bt2020,
        ColorSpace::Smpte240 => YuvStandardMatrix::Smpte240,
        _ => YuvStandardMatrix::Bt601,
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
        let mut image = image::RgbaImage::from_pixel(5, 1, image::Rgba([60, 120, 180, 255]));
        let alpha = image::GrayImage::from_raw(5, 1, vec![0, 16, 128, 235, 255]).unwrap();
        apply_alpha(&mut image, &alpha).unwrap();
        for (pixel, sample) in image.pixels().zip(alpha.pixels()) {
            assert_eq!(pixel.0, [60, 120, 180, sample[0]]);
        }
        assert!(apply_alpha(&mut image, &image::GrayImage::new(4, 1)).is_err());
    }
}
