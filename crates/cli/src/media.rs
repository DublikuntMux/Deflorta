//! Normalize media bytes while preserving the game's logical asset paths.

use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use deflorta_assets::GameFiles;
use serde::Deserialize;
use tempfile::TempDir;

use crate::project::Project;

#[derive(Clone, Copy)]
enum Kind {
    Image,
    Video,
    Audio,
}

impl Kind {
    fn of(path: &str) -> Option<Self> {
        let extension = Path::new(path).extension()?.to_str()?.to_ascii_lowercase();
        Some(match extension.as_str() {
            "png" | "jpg" | "jpeg" | "webp" | "gif" | "avif" | "bmp" | "tif" | "tiff" | "tga"
            | "dds" | "ico" | "exr" | "hdr" | "qoi" | "jxl" | "pnm" | "ppm" | "pgm" | "pbm"
            | "pam" | "psd" | "psb" | "heic" | "heif" | "svg" | "apng" | "jfif" | "jpe" | "j2k"
            | "jp2" | "jpf" | "jpx" | "pcx" | "xbm" | "xpm" | "xwd" | "dib" => Self::Image,
            "mp4" | "webm" | "mkv" | "mov" | "avi" | "m4v" | "mpg" | "mpeg" | "ogv" | "wmv"
            | "flv" | "ts" | "mts" | "m2ts" | "3gp" | "3g2" | "vob" | "m2v" | "m2p" | "divx"
            | "asf" | "rm" | "rmvb" | "f4v" | "mxf" | "nut" => Self::Video,
            "wav" | "mp3" | "ogg" | "oga" | "opus" | "flac" | "aac" | "m4a" | "aif" | "aiff"
            | "wma" | "weba" | "amr" | "ac3" | "caf" | "au" | "snd" | "mka" | "ape" | "m4b"
            | "mp2" | "mp1" | "ra" | "w64" | "wv" | "tta" | "spx" | "aifc" | "voc" | "dts" => {
                Self::Audio
            }
            _ => return None,
        })
    }

    const fn options(self) -> &'static [&'static str] {
        match self {
            Self::Image => &[
                "-map",
                "0:v:0",
                "-frames:v",
                "1",
                "-an",
                "-c:v",
                "libwebp",
                "-lossless",
                "1",
                "-compression_level",
                "6",
                "-f",
                "webp",
            ],
            Self::Video => &[
                "-map",
                "0:V:0",
                "-map",
                "0:a:0?",
                "-c:v",
                "libvpx-vp9",
                "-pix_fmt",
                "yuv420p",
                "-b:v",
                "0",
                "-crf",
                "30",
                "-deadline",
                "good",
                "-cpu-used",
                "2",
                "-c:a",
                "libvorbis",
                "-q:a",
                "5",
                "-ac",
                "2",
                "-f",
                "webm",
            ],
            Self::Audio => &[
                "-map",
                "0:a:0",
                "-vn",
                "-c:a",
                "libvorbis",
                "-q:a",
                "5",
                "-ac",
                "2",
                "-f",
                "ogg",
            ],
        }
    }
}

/// Owns the staging directory until its archive is packed.
pub struct PreparedAssets {
    pub files: GameFiles,
    _directory: TempDir,
}

pub fn is_media(path: &str) -> bool {
    Kind::of(path).is_some()
}

pub fn prepare(project: &Project, paths: &[String]) -> Result<PreparedAssets> {
    let directory = tempfile::Builder::new()
        .prefix("deflorta-media-")
        .tempdir()?;
    let input = tempfile::Builder::new()
        .prefix("deflorta-source-")
        .tempdir()?;
    for path in paths {
        let output = directory.path().join(path);
        std::fs::create_dir_all(output.parent().context("asset has no parent")?)?;
        let mut source = project.files.open_file(path)?;
        if let Some(kind) = Kind::of(path) {
            // Read through GameFiles so project symlinks cannot bypass its sandbox.
            let source_path = input.path().join(
                Path::new(path)
                    .file_name()
                    .context("asset has no file name")?,
            );
            io::copy(&mut source, &mut std::fs::File::create(&source_path)?)?;
            eprintln!("Converting {path}");
            transcode(&source_path, &output, kind)
                .with_context(|| format!("cannot convert media asset '{path}'"))?;
        } else {
            io::copy(&mut source, &mut std::fs::File::create(&output)?)?;
        }
    }
    Ok(PreparedAssets {
        files: GameFiles::directory(directory.path())?,
        _directory: directory,
    })
}

fn transcode(input: &Path, output: &Path, kind: Kind) -> Result<()> {
    let kind = match kind {
        Kind::Image => Kind::Image,
        Kind::Video | Kind::Audio => stream_kind(input)?,
    };
    let result = Command::new("ffmpeg")
        .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(input)
        .args(["-map_metadata", "-1", "-map_chapters", "-1"])
        .args(kind.options())
        .arg(output)
        .stdin(Stdio::null())
        .output()
        .context("cannot start FFmpeg; install ffmpeg with libwebp, libvpx-vp9, and libvorbis encoders on PATH")?;
    if !result.status.success() {
        bail!(
            "FFmpeg failed ({}): {}",
            result.status,
            String::from_utf8_lossy(&result.stderr).trim()
        );
    }
    Ok(())
}

#[derive(Deserialize)]
struct Probe {
    streams: Vec<Stream>,
}

#[derive(Deserialize)]
struct Stream {
    codec_type: String,
    #[serde(default)]
    disposition: Disposition,
}

#[derive(Default, Deserialize)]
struct Disposition {
    #[serde(default)]
    attached_pic: u8,
}

/// MP4 can hold only audio, and Ogg can hold video. Album art is not a movie.
fn stream_kind(input: &Path) -> Result<Kind> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_type:stream_disposition=attached_pic",
            "-of",
            "json",
        ])
        .arg(input)
        .stdin(Stdio::null())
        .output()
        .context("cannot start ffprobe; install the FFmpeg tools on PATH")?;
    if !output.status.success() {
        bail!(
            "ffprobe failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let probe: Probe = serde_json::from_slice(&output.stdout)
        .context("ffprobe returned invalid stream metadata")?;
    if probe
        .streams
        .iter()
        .any(|stream| stream.codec_type == "video" && stream.disposition.attached_pic == 0)
    {
        return Ok(Kind::Video);
    }
    if probe
        .streams
        .iter()
        .any(|stream| stream.codec_type == "audio")
    {
        return Ok(Kind::Audio);
    }
    bail!("media has no video or audio stream")
}
