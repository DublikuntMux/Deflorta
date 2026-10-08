//! Normalize media bytes while preserving the game's logical asset paths.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::SystemTime;

use anyhow::{Context, Result, bail};
use deflorta_assets::GameFiles;
use serde::{Deserialize, Serialize};
use tempfile::TempDir;

use crate::project::Project;

// Bump when the conversion options or stream selection change.
const CACHE_VERSION: u32 = 1;
const CACHE_INDEX: &str = ".cache/media.json";
const CACHE_MEDIA: &str = ".cache/media";

#[derive(Deserialize, Serialize)]
struct Cache {
    version: u32,
    entries: BTreeMap<String, CachedAsset>,
}

impl Default for Cache {
    fn default() -> Self {
        Self {
            version: CACHE_VERSION,
            entries: BTreeMap::new(),
        }
    }
}

impl Cache {
    fn load(project: &Project) -> Result<Self> {
        let file = match project.files.open_file(CACHE_INDEX) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error).context("cannot read media cache index"),
        };
        // A corrupt or obsolete index is disposable; rebuild it from sources.
        Ok(serde_json::from_reader::<_, Self>(file)
            .ok()
            .filter(|cache| cache.version == CACHE_VERSION)
            .unwrap_or_default())
    }

    fn save(&self, project: &Project) -> Result<()> {
        let index = project.dir.join(CACHE_INDEX);
        let mut temporary =
            tempfile::NamedTempFile::new_in(index.parent().context("cache index has no parent")?)?;
        serde_json::to_writer(temporary.as_file_mut(), self)?;
        temporary
            .persist(index)
            .context("cannot save media cache index")?;
        Ok(())
    }
}

#[derive(Deserialize, PartialEq, Serialize)]
struct CachedAsset {
    modified: SystemTime,
    size: u64,
}

impl CachedAsset {
    fn source(project: &Project, path: &str) -> Result<Self> {
        let metadata = std::fs::metadata(project.dir.join(path))?;
        Ok(Self {
            modified: metadata.modified()?,
            size: metadata.len(),
        })
    }
}

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
    let mut cache = Cache::load(project)?;
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
            let fingerprint = CachedAsset::source(project, path)?;
            let cached_path = format!("{CACHE_MEDIA}/{path}");
            if cache.entries.get(path) == Some(&fingerprint)
                && let Ok(mut cached) = project.files.open_file(&cached_path)
            {
                io::copy(&mut cached, &mut std::fs::File::create(&output)?)?;
                continue;
            }
            // Read through GameFiles so project symlinks cannot bypass its sandbox.
            let source_path = input.path().join(
                Path::new(path)
                    .file_name()
                    .context("asset has no file name")?,
            );
            io::copy(&mut source, &mut std::fs::File::create(&source_path)?)?;
            let cached_output = project.dir.join(&cached_path);
            let parent = cached_output
                .parent()
                .context("cached asset has no parent")?;
            std::fs::create_dir_all(parent)?;
            let temporary = tempfile::NamedTempFile::new_in(parent)?;
            eprintln!("Converting {path}");
            transcode(&source_path, temporary.path(), kind)
                .with_context(|| format!("cannot convert media asset '{path}'"))?;
            temporary
                .persist(&cached_output)
                .with_context(|| format!("cannot cache media asset '{path}'"))?;
            cache.entries.insert(path.clone(), fingerprint);
            // Keep successful conversions even if a later asset fails.
            cache.save(project)?;
            std::fs::copy(cached_output, &output)?;
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

#[cfg(test)]
mod tests {
    use std::fs::{File, FileTimes};
    use std::time::Duration;

    use super::*;

    // Invalid source bytes ensure a cache hit never invokes the encoder.
    fn cached_project() -> (TempDir, Project, Vec<String>) {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("main.js"), "").unwrap();
        let project = Project::open(directory.path()).unwrap();
        let paths = vec!["audio/scene.wav".to_owned(), "images/scene.png".to_owned()];
        let mut cache = Cache::default();
        for path in &paths {
            let source = project.dir.join(path);
            std::fs::create_dir_all(source.parent().unwrap()).unwrap();
            std::fs::write(source, b"invalid source").unwrap();
            let output = project.dir.join(CACHE_MEDIA).join(path);
            std::fs::create_dir_all(output.parent().unwrap()).unwrap();
            std::fs::write(output, format!("cached {path}")).unwrap();
            cache
                .entries
                .insert(path.clone(), CachedAsset::source(&project, path).unwrap());
        }
        cache.save(&project).unwrap();
        (directory, project, paths)
    }

    #[test]
    fn persistent_cache_reuses_media_and_is_excluded_from_assets() {
        let (_directory, project, mut paths) = cached_project();
        std::fs::write(project.dir.join("notes.txt"), "original text").unwrap();
        paths.push("notes.txt".into());
        assert_eq!(project.assets(&[]).unwrap(), paths);
        for _ in 0..2 {
            let prepared = prepare(&Project::open(&project.dir).unwrap(), &paths).unwrap();
            for path in &paths[..2] {
                assert_eq!(
                    prepared.files.read_to_string(path).unwrap(),
                    format!("cached {path}")
                );
            }
            assert_eq!(
                prepared.files.read_to_string("notes.txt").unwrap(),
                "original text"
            );
        }
    }

    #[test]
    fn changed_modification_time_reencodes_and_failure_preserves_cache() {
        let (_directory, project, paths) = cached_project();
        let path = &paths[0];
        let modified = CachedAsset::source(&project, path).unwrap().modified;
        File::open(project.dir.join(path))
            .unwrap()
            .set_times(FileTimes::new().set_modified(modified + Duration::from_secs(1)))
            .unwrap();
        let error = prepare(&project, &paths).err().unwrap();
        assert!(error.to_string().contains("cannot convert media asset"));
        assert_eq!(
            Cache::load(&project).unwrap().entries[path].modified,
            modified
        );
        assert_eq!(
            std::fs::read_to_string(project.dir.join(CACHE_MEDIA).join(path)).unwrap(),
            format!("cached {path}")
        );
    }

    #[test]
    fn changed_size_reencodes_even_when_modification_time_matches() {
        let (_directory, project, paths) = cached_project();
        let path = &paths[0];
        let modified = CachedAsset::source(&project, path).unwrap().modified;
        std::fs::write(project.dir.join(path), b"different invalid source").unwrap();
        File::open(project.dir.join(path))
            .unwrap()
            .set_times(FileTimes::new().set_modified(modified))
            .unwrap();
        assert!(prepare(&project, &paths).is_err());
    }

    #[test]
    fn missing_cached_output_reencodes() {
        let (_directory, project, paths) = cached_project();
        std::fs::remove_file(project.dir.join(CACHE_MEDIA).join(&paths[0])).unwrap();
        assert!(prepare(&project, &paths).is_err());
    }

    #[test]
    fn invalid_missing_or_obsolete_index_reencodes() {
        let (_directory, project, paths) = cached_project();
        let mut cache = Cache::load(&project).unwrap();
        cache.version += 1;
        cache.save(&project).unwrap();
        assert!(prepare(&project, &paths).is_err());
        std::fs::write(project.dir.join(CACHE_INDEX), b"{broken json").unwrap();
        assert!(prepare(&project, &paths).is_err());
        std::fs::remove_file(project.dir.join(CACHE_INDEX)).unwrap();
        assert!(prepare(&project, &paths).is_err());
    }
}
