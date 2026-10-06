//! Writes `.dm` archives (format in `deflorta::archive`). Each file is split
//! into blocks that are compressed with LZ4HC in parallel; blocks that do not
//! shrink, and files in already-compressed formats, are stored as they are.

use std::fs::File;
use std::io::{BufWriter, Cursor, Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use deflorta::archive::{BLOCK_COMPRESSED, BLOCK_SIZE, Block, Entry, Index, encode_header};
use lz4::block::CompressionMode;

/// Default LZ4HC level (LZ4's own default; 12 is the maximum).
pub const DEFAULT_LEVEL: i32 = 9;

/// Extensions of formats that are already compressed.
const COMPRESSED_FORMATS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "gif", "avif", "mp4", "m4a", "webm", "mkv", "ogg", "oga", "opus",
    "mp3", "flac", "aac", "zip", "gz", "woff2",
];

pub enum Contents {
    File(PathBuf),
    Bytes(Vec<u8>),
}

pub struct ArchiveFile {
    /// Path inside the archive (`images/bg room.png`).
    pub path: String,
    pub contents: Contents,
}

#[derive(Default)]
pub struct Stats {
    pub files: usize,
    pub size: u64,
    pub stored: u64,
}

fn compress_block(data: &[u8], level: Option<i32>) -> Result<(Vec<u8>, u8)> {
    if let Some(level) = level {
        let compressed =
            lz4::block::compress(data, Some(CompressionMode::HIGHCOMPRESSION(level)), false)?;
        if compressed.len() < data.len() {
            return Ok((compressed, BLOCK_COMPRESSED));
        }
    }
    Ok((data.to_vec(), 0))
}

/// Compresses blocks on all cores, preserving their order.
fn compress_blocks(data: &[u8], level: Option<i32>) -> Result<Vec<(Vec<u8>, u8)>> {
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
                        .map(|block| compress_block(block, level))
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
pub fn write_archive(output: &Path, mut files: Vec<ArchiveFile>, level: i32) -> Result<Stats> {
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
            Contents::File(path) => Box::new(
                File::open(&path).with_context(|| format!("cannot read {}", path.display()))?,
            ),
            Contents::Bytes(bytes) => Box::new(Cursor::new(bytes)),
        };
        let level = (!is_compressed_format(&file.path)).then_some(level);
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
            for (stored, flags) in compress_blocks(&bytes, level)? {
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
        raw_index.len() <= deflorta::archive::MAX_INDEX_SIZE as usize,
        "archive index is too large"
    );
    let stored_index = lz4::block::compress(
        &raw_index,
        Some(CompressionMode::HIGHCOMPRESSION(level)),
        false,
    )?;
    ensure!(
        stored_index.len() <= deflorta::archive::MAX_INDEX_SIZE as usize,
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
    stats.stored += (deflorta::archive::HEADER_SIZE + stored_index.len()) as u64;
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Seek, SeekFrom};
    use std::sync::Arc;

    use deflorta::archive::{Archive, MAX_INDEX_SIZE};

    use super::*;

    #[test]
    fn round_trips_files_and_seeks_across_blocks() {
        let dir = std::env::temp_dir().join(format!("deflorta-pack-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let output = dir.join("game.dm");
        // Compressible text spanning several blocks, incompressible noise, and an empty file.
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
        let source = dir.join("source.js");
        std::fs::write(&source, &text).unwrap();
        let files = vec![
            ArchiveFile {
                path: "main.js".into(),
                contents: Contents::File(source),
            },
            ArchiveFile {
                path: "audio/noise.bin".into(),
                contents: Contents::Bytes(noise.clone()),
            },
            ArchiveFile {
                path: "empty.txt".into(),
                contents: Contents::Bytes(Vec::new()),
            },
        ];
        let archive_stats = write_archive(&output, files, DEFAULT_LEVEL).unwrap();
        assert_eq!(archive_stats.files, 3);
        assert!(archive_stats.stored < archive_stats.size);

        let archive = Arc::new(Archive::open(&output).unwrap());
        assert_eq!(archive.read("main.js").unwrap(), text);
        assert_eq!(archive.read("audio/noise.bin").unwrap(), noise);
        assert_eq!(archive.read("empty.txt").unwrap(), [] as [u8; 0]);
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

        let files = deflorta::GameFiles::open(&output).unwrap();
        assert_eq!(files.list("audio"), ["audio/noise.bin"]);
        assert!(files.exists("./main.js"));
        assert!(files.read("../main.js").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
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
            let stored = lz4::block::compress(&raw, None, false).unwrap();
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
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
