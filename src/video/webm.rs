//! Pure Rust `WebM` demuxing (Symphonia) and VP8/VP9 decoding (`OxideAV`).

use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::SyncSender;

use anyhow::{Context, Result, bail};
use log::trace;
use oxideav_vp8::state::Vp8DecoderState;
use oxideav_vp9::{
    ColorConfig, ColorSpace, FrameType, Vp9DecodedFrame, Vp9SequenceDecoder,
    parse_uncompressed_header,
};
use symphonia::core::codecs::{
    CodecParameters,
    video::well_known::{CODEC_ID_VP8, CODEC_ID_VP9},
};
use symphonia::core::formats::{FormatReader, Track};
use yuv::{YuvPlanarImage, YuvRange, YuvStandardMatrix};

use super::{Frame, Message};
use crate::media;

fn video_track(reader: &dyn FormatReader) -> Result<&Track> {
    reader.tracks().iter().find(|track| matches!(
        &track.codec_params,
        Some(CodecParameters::Video(params)) if params.codec == CODEC_ID_VP8 || params.codec == CODEC_ID_VP9
    )).context("no supported WebM video track (expected VP8 or VP9)")
}

pub(super) fn probe_size(path: &Path) -> Result<(u32, u32)> {
    let reader = media::open(path)?;
    let track = video_track(reader.as_ref())?;
    let Some(CodecParameters::Video(params)) = &track.codec_params else {
        unreachable!()
    };
    Ok((
        u32::from(params.width.context("WebM video width is missing")?),
        u32::from(params.height.context("WebM video height is missing")?),
    ))
}

pub(super) fn decode(
    path: &Path,
    looping: bool,
    sender: &SyncSender<Message>,
) -> Result<(u64, f64)> {
    let mut offset = 0.0;
    let mut frames = 0;
    loop {
        let mut reader = media::open(path)?;
        let track = video_track(reader.as_ref())?;
        let track_id = track.id;
        let time_base = track.time_base.context("WebM video time base is missing")?;
        let Some(CodecParameters::Video(params)) = &track.codec_params else {
            unreachable!()
        };
        let vp9 = params.codec == CODEC_ID_VP9;
        let mut vp8_decoder = Vp8DecoderState::new();
        let mut vp9_decoder = Vp9SequenceDecoder::new();
        let mut color = ColorConfig {
            bit_depth: 8,
            color_space: ColorSpace::Bt601,
            color_range_full: false,
            subsampling_x: true,
            subsampling_y: true,
        };
        let mut end = media::duration(reader.as_ref()).unwrap_or(0.0);
        let mut last_pts: Option<f64> = None;
        let mut last_interval = 0.0;
        let first_frame = frames;
        while let Some(packet) = reader.next_packet()? {
            if packet.track_id != track_id {
                continue;
            }
            let pts = time_base
                .calc_time(packet.pts)
                .context("WebM timestamp overflow")?
                .as_secs_f64();
            let duration = time_base
                .calc_duration(packet.dur)
                .context("WebM duration overflow")?
                .as_secs_f64();
            if let Some(previous) = last_pts {
                last_interval = (pts - previous).max(0.0);
            }
            last_pts = Some(pts);
            end = end.max(pts + duration);
            let mut images = Vec::new();
            if vp9 {
                // A WebM packet may contain a hidden reference frame followed by a shown frame.
                for bytes in oxideav_vp9::split_superframe(&packet.data) {
                    // Inter frames inherit the last intra frame's color configuration.
                    if let Ok(header) = parse_uncompressed_header(bytes)
                        && !header.show_existing_frame
                        && (header.frame_type == FrameType::KeyFrame || header.intra_only)
                    {
                        color = header.color_config;
                    }
                    if let Some(decoded) = vp9_decoder.push_frame(bytes)? {
                        images.push(vp9_rgba(&decoded, color)?);
                    }
                }
            } else {
                let decoded = vp8_decoder.decode_frame(&packet.data)?;
                if vp8_decoder.last_frame_shown() == Some(true) {
                    images.push(to_rgba(
                        decoded.width,
                        decoded.height,
                        [&decoded.y, &decoded.u, &decoded.v],
                        (true, true),
                        color,
                    )?);
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
        trace!("Looping video {}", path.display());
        offset += end;
    }
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
