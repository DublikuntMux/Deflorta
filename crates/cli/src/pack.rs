use std::fs::File;
use std::io::{BufWriter, Cursor, Read, Write};
use std::path::Path;

use anyhow::{Context, Result, ensure};
use deflorta_data::archive::{BLOCK_COMPRESSED, BLOCK_SIZE, Block, Entry, Index, encode_header};
use lz4::liblz4::BlockChecksum;
use lz4::{BlockMode, BlockSize, ContentChecksum, EncoderBuilder};

pub const DEFAULT_LEVEL: u8 = 12;

const COMPRESSED_FORMATS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "gif", "avif", "mp4", "m4a", "webm", "mkv", "ogg", "oga", "opus",
    "mp3", "flac", "aac", "zip", "gz", "woff2",
];

pub enum Contents {
    File(deflorta_data::GameFiles),
    Bytes(Vec<u8>),
}

pub struct ArchiveFile {
    pub path: String,
    pub contents: Contents,
}

#[derive(Default)]
pub struct Stats {
    pub files: usize,
    pub size: u64,
    pub stored: u64,
}

/// Encode standard independent-block LZ4 frames with content size and checksum.
fn compress_frame(data: &[u8], level: u8) -> Result<Vec<u8>> {
    let mut encoder = EncoderBuilder::new()
        .block_size(BlockSize::Max256KB)
        .block_mode(BlockMode::Independent)
        .block_checksum(BlockChecksum::NoBlockChecksum)
        .checksum(ContentChecksum::ChecksumEnabled)
        .content_size(u64::try_from(data.len())?)
        .level(u32::from(level))
        .build(Vec::new())?;
    encoder.write_all(data)?;
    let (frame, result) = encoder.finish();
    result?;
    Ok(frame)
}

fn compress_block(data: &[u8], compress: bool, level: u8) -> Result<(Vec<u8>, u8)> {
    if compress {
        let compressed = compress_frame(data, level)?;
        if compressed.len() < data.len() {
            return Ok((compressed, BLOCK_COMPRESSED));
        }
    }
    Ok((data.to_vec(), 0))
}

fn compress_blocks(data: &[u8], compress: bool, level: u8) -> Result<Vec<(Vec<u8>, u8)>> {
    let blocks: Vec<&[u8]> = data.chunks(BLOCK_SIZE as usize).collect();
    let threads = std::thread::available_parallelism()
        .map_or(1, std::num::NonZero::get)
        .min(blocks.len().max(1));
    let per_thread = blocks.len().div_ceil(threads.max(1)).max(1);
    std::thread::scope(|scope| {
        let workers: Vec<_> = blocks
            .chunks(per_thread)
            .map(|group| {
                scope.spawn(move || {
                    group
                        .iter()
                        .map(|block| compress_block(block, compress, level))
                        .collect::<Result<Vec<_>>>()
                })
            })
            .collect();
        let mut out = Vec::with_capacity(blocks.len());
        for worker in workers {
            out.extend(worker.join().expect("compression thread panicked")?);
        }
        Ok(out)
    })
}

fn is_compressed_format(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| COMPRESSED_FORMATS.iter().any(|f| e.eq_ignore_ascii_case(f)))
}

/// Writes `files` to `output` atomically.
pub fn write_archive(output: &Path, mut files: Vec<ArchiveFile>, level: u8) -> Result<Stats> {
    ensure!(
        (1..=12).contains(&level),
        "LZ4 compression level must be 1–12"
    );
    files.sort_by(|a, b| a.path.cmp(&b.path));
    if let Some(dir) = output.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let data_path = output.with_extension("dm.data");
    let mut data = BufWriter::new(
        File::create(&data_path)
            .with_context(|| format!("cannot create {}", data_path.display()))?,
    );
    let mut index = Index::default();
    let mut stats = Stats::default();
    // Compress bounded groups of blocks so a large movie does not need to
    // fit in memory. Parallelism is limited by the blocks in each group.
    let batch_size = u64::from(BLOCK_SIZE) * 32;
    for file in files {
        let mut input: Box<dyn Read> = match file.contents {
            Contents::File(files) => Box::new(
                files
                    .open_file(&file.path)
                    .with_context(|| format!("cannot read {}", file.path))?,
            ),
            Contents::Bytes(bytes) => Box::new(Cursor::new(bytes)),
        };
        let compress = !is_compressed_format(&file.path);
        let mut entry = Entry {
            path: file.path,
            size: 0,
            first_block: u32::try_from(index.blocks.len()).context("too many blocks")?,
        };
        loop {
            let mut bytes = Vec::new();
            input.by_ref().take(batch_size).read_to_end(&mut bytes)?;
            if bytes.is_empty() {
                break;
            }
            entry.size += bytes.len() as u64;
            for (stored, flags) in compress_blocks(&bytes, compress, level)? {
                data.write_all(&stored)?;
                stats.stored += stored.len() as u64;
                index.blocks.push(Block {
                    stored_size: u32::try_from(stored.len())?,
                    flags,
                });
            }
        }
        stats.files += 1;
        stats.size += entry.size;
        index.entries.push(entry);
    }
    data.flush()?;
    drop(data);

    let raw_index = index.encode()?;
    ensure!(
        raw_index.len() <= deflorta_data::archive::MAX_INDEX_SIZE as usize,
        "archive index is too large"
    );
    let stored_index = compress_frame(&raw_index, level)?;
    ensure!(
        stored_index.len() <= deflorta_data::archive::MAX_INDEX_SIZE as usize,
        "archive index is too large"
    );
    let partial = output.with_extension("dm.partial");
    {
        let mut out = BufWriter::new(
            File::create(&partial)
                .with_context(|| format!("cannot create {}", partial.display()))?,
        );
        out.write_all(&encode_header(
            BLOCK_SIZE,
            u32::try_from(stored_index.len())?,
            u32::try_from(raw_index.len())?,
        ))?;
        out.write_all(&stored_index)?;
        std::io::copy(&mut File::open(&data_path)?, &mut out)?;
        out.flush()?;
    }
    std::fs::remove_file(&data_path)?;
    std::fs::rename(&partial, output)
        .with_context(|| format!("cannot write {}", output.display()))?;
    stats.stored += (deflorta_data::archive::HEADER_SIZE + stored_index.len()) as u64;
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Seek, SeekFrom};
    use std::sync::Arc;

    use deflorta_data::archive::{Archive, MAX_INDEX_SIZE, decompress_frame};

    use super::*;

    #[test]
    fn empty_frames_require_a_complete_footer_and_no_trailing_data() {
        let frame = compress_frame(&[], DEFAULT_LEVEL).unwrap();
        assert_eq!(decompress_frame(&frame, 0).unwrap(), [] as [u8; 0]);
        assert!(decompress_frame(&frame[..frame.len() - 1], 0).is_err());
        let mut trailing = frame;
        trailing.push(0);
        assert!(decompress_frame(&trailing, 0).is_err());
    }

    #[test]
    fn hc_frames_round_trip_multiple_internal_blocks_and_verify_checksums() {
        let data = b"LZ4HC frames preserve seekable game resources.\n".repeat(12_000);
        for level in [1, DEFAULT_LEVEL, 12] {
            let frame = compress_frame(&data, level).unwrap();
            assert_eq!(&frame[..4], &0x184D_2204u32.to_le_bytes());
            assert!(
                frame.len() < data.len(),
                "level {}: {} bytes for {} bytes of input",
                level,
                frame.len(),
                data.len()
            );
            assert_eq!(decompress_frame(&frame, data.len()).unwrap(), data);
            assert!(decompress_frame(&frame, data.len() - 1).is_err());
            assert!(decompress_frame(&frame, data.len() + 1).is_err());
            assert!(decompress_frame(&frame[..frame.len() - 1], data.len()).is_err());
            let mut trailing = frame;
            trailing.push(0);
            assert!(decompress_frame(&trailing, data.len()).is_err());
        }
        let mut frame = compress_frame(&data, DEFAULT_LEVEL).unwrap();
        let checksum = frame.len() - 1;
        frame[checksum] ^= 1;
        assert!(decompress_frame(&frame, data.len()).is_err());
    }

    #[test]
    fn round_trips_files_and_seeks_across_blocks() {
        let dir = std::env::temp_dir().join(format!("deflorta-pack-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let output = dir.join("game.dm");
        let text: Vec<u8> = (0..500_000u32)
            .flat_map(|i| format!("line {i}\n").into_bytes())
            .collect();
        let mut state = 0x1234_5678u32;
        let noise: Vec<u8> = (0..200_000)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state.to_le_bytes()[0]
            })
            .collect();
        let source = dir.join("main.js");
        std::fs::write(&source, &text).unwrap();
        let files = vec![
            ArchiveFile {
                path: "main.js".into(),
                contents: Contents::File(deflorta_data::GameFiles::directory(&dir).unwrap()),
            },
            ArchiveFile {
                path: "audio/noise.bin".into(),
                contents: Contents::Bytes(noise.clone()),
            },
            ArchiveFile {
                path: "images/already-compressed.PNG".into(),
                contents: Contents::Bytes(vec![b'x'; 1000]),
            },
            ArchiveFile {
                path: "empty.txt".into(),
                contents: Contents::Bytes(Vec::new()),
            },
        ];
        let archive_stats = write_archive(&output, files, DEFAULT_LEVEL).unwrap();
        assert_eq!(archive_stats.files, 4);
        assert!(archive_stats.stored < archive_stats.size);

        let bytes = std::fs::read(&output).unwrap();
        assert_eq!(&bytes[8..12], &2u32.to_le_bytes());
        assert_eq!(
            &bytes[deflorta_data::archive::HEADER_SIZE..][..4],
            &0x184D_2204u32.to_le_bytes()
        );

        let archive = Arc::new(Archive::open(&output).unwrap());
        assert_eq!(archive.read("main.js").unwrap(), text);
        assert_eq!(archive.read("audio/noise.bin").unwrap(), noise);
        assert_eq!(archive.read("empty.txt").unwrap(), [] as [u8; 0]);
        assert_eq!(
            archive.read("images/already-compressed.PNG").unwrap(),
            vec![b'x'; 1000]
        );
        assert_eq!(
            archive.stored_size("images/already-compressed.PNG"),
            Some(1000)
        );
        assert!(archive.read("missing").is_err());

        let mut reader = archive.open_entry("main.js").unwrap();
        let offset = u64::from(BLOCK_SIZE) * 2 - 3;
        reader.seek(SeekFrom::Start(offset)).unwrap();
        let mut buf = [0; 10];
        reader.read_exact(&mut buf).unwrap();
        let start = usize::try_from(offset).unwrap();
        assert_eq!(&buf, &text[start..start + 10]);
        reader.seek(SeekFrom::End(-4)).unwrap();
        let mut tail = Vec::new();
        reader.read_to_end(&mut tail).unwrap();
        assert_eq!(tail, &text[text.len() - 4..]);

        let files = deflorta_data::GameFiles::open(&output).unwrap();
        assert_eq!(files.list("audio"), ["audio/noise.bin"]);
        assert!(files.exists("./main.js"));
        assert!(files.read("../main.js").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rejects_archives_using_the_previous_raw_block_format() {
        // Captured from the previous CLI (native LZ4HC level 9). Includes a compressed index,
        // a compressed text block and a verbatim script block, using format v1.
        const LEGACY: &[u8] = &[
            0x44, 0x45, 0x46, 0x4c, 0x4f, 0x52, 0x54, 0x41, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x02, 0x00, 0x38, 0x00, 0x00, 0x00, 0x3f, 0x00, 0x00, 0x00, 0xe0, 0x02, 0x00, 0x00,
            0x00, 0x22, 0x00, 0x00, 0x00, 0x01, 0x38, 0x00, 0x00, 0x00, 0x00, 0x0e, 0x00, 0xf5,
            0x00, 0x0a, 0x00, 0x6c, 0x65, 0x67, 0x61, 0x63, 0x79, 0x2e, 0x74, 0x78, 0x74, 0xb8,
            0x01, 0x00, 0x01, 0x00, 0x91, 0x07, 0x00, 0x6d, 0x61, 0x69, 0x6e, 0x2e, 0x6a, 0x73,
            0x2a, 0x00, 0x70, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0xff, 0x07, 0x4c, 0x65,
            0x67, 0x61, 0x63, 0x79, 0x20, 0x4c, 0x5a, 0x34, 0x48, 0x43, 0x20, 0x61, 0x72, 0x63,
            0x68, 0x69, 0x76, 0x65, 0x2e, 0x0a, 0x16, 0x00, 0xff, 0x8b, 0x50, 0x69, 0x76, 0x65,
            0x2e, 0x0a, 0x69, 0x6d, 0x70, 0x6f, 0x72, 0x74, 0x7b, 0x6c, 0x61, 0x62, 0x65, 0x6c,
            0x20, 0x61, 0x73, 0x20, 0x65, 0x7d, 0x66, 0x72, 0x6f, 0x6d, 0x22, 0x64, 0x65, 0x66,
            0x6c, 0x6f, 0x72, 0x74, 0x61, 0x22, 0x3b, 0x65, 0x28, 0x60, 0x73, 0x74, 0x61, 0x72,
            0x74, 0x60, 0x2c, 0x61, 0x73, 0x79, 0x6e, 0x63, 0x28, 0x29, 0x3d, 0x3e, 0x7b, 0x7d,
            0x29, 0x3b,
        ];
        let output =
            std::env::temp_dir().join(format!("deflorta-legacy-{}.dm", std::process::id()));
        std::fs::write(&output, LEGACY).unwrap();
        let error = Archive::open(&output)
            .err()
            .expect("version 1 must be rejected");
        assert!(error.to_string().contains("unsupported archive version 1"));
        std::fs::remove_file(output).unwrap();
    }

    #[test]
    fn rejects_corrupt_indexes_paths_flags_and_truncated_blocks() {
        let dir = std::env::temp_dir().join(format!("deflorta-corrupt-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let output = dir.join("game.dm");
        let entry = Entry {
            path: "main.js".into(),
            size: 1,
            first_block: 0,
        };
        let valid = Index {
            entries: vec![entry.clone()],
            blocks: vec![Block {
                stored_size: 1,
                flags: 0,
            }],
        };
        let write = |index: &Index, payload: &[u8], decoded_size: Option<u32>| {
            let raw = index.encode().unwrap();
            let stored = compress_frame(&raw, DEFAULT_LEVEL).unwrap();
            let mut bytes = encode_header(
                BLOCK_SIZE,
                u32::try_from(stored.len()).unwrap(),
                decoded_size.unwrap_or_else(|| u32::try_from(raw.len()).unwrap()),
            )
            .to_vec();
            bytes.extend(stored);
            bytes.extend(payload);
            std::fs::write(&output, bytes).unwrap();
        };
        write(&valid, b"x", None);
        assert_eq!(
            Archive::open(&output).unwrap().read("main.js").unwrap(),
            b"x"
        );
        write(&valid, b"", None);
        assert!(Archive::open(&output).is_err());
        write(&valid, b"x", Some(MAX_INDEX_SIZE + 1));
        assert!(Archive::open(&output).is_err());
        let mut invalid = valid.clone();
        invalid.entries.push(entry);
        write(&invalid, b"x", None);
        assert!(Archive::open(&output).is_err());
        for path in ["../escape", "/absolute", "./main.js"] {
            invalid = valid.clone();
            invalid.entries[0].path = path.into();
            write(&invalid, b"x", None);
            assert!(Archive::open(&output).is_err(), "accepted {path}");
        }
        invalid = valid;
        invalid.blocks[0].flags = 0x80;
        write(&invalid, b"x", None);
        assert!(Archive::open(&output).is_err());

        invalid.blocks[0].flags = BLOCK_COMPRESSED;
        let mut frame = compress_frame(b"x", DEFAULT_LEVEL).unwrap();
        invalid.blocks[0].stored_size = u32::try_from(frame.len()).unwrap();
        let checksum = frame.len() - 1;
        frame[checksum] ^= 1;
        write(&invalid, &frame, None);
        assert!(Archive::open(&output).unwrap().read("main.js").is_err());

        frame[checksum] ^= 1;
        write(&invalid, &frame, None);
        let mut bytes = std::fs::read(&output).unwrap();
        // Corrupt the framed index's content checksum, immediately before the payload.
        let index_checksum = bytes.len() - frame.len() - 1;
        bytes[index_checksum] ^= 1;
        std::fs::write(&output, bytes).unwrap();
        assert!(Archive::open(&output).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
