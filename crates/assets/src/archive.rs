//! Version 2 `.dm` archive format; all integers are little-endian.
//!
//! ```text
//! header   magic "DEFLORTA" | version u32 | block size u32
//!          | stored index size u32 | index size u32
//! index    LZ4 frame:
//!          block count u32, then per block: stored size u32, flags u8
//!          entry count u32, then per entry: path length u16, UTF-8 path,
//!          size u64, first block u32
//! blocks   each file is split into `block size` chunks, each compressed chunk
//!          is an independent LZ4 frame with a content checksum
//! ```
//!
//! [`BLOCK_COMPRESSED`] marks LZ4 frames; other blocks are verbatim.
//! A file's blocks are consecutive.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, bail, ensure};

pub const MAGIC: [u8; 8] = *b"DEFLORTA";
pub const VERSION: u32 = 2;
/// Uncompressed size of every block except the last block of a file.
pub const BLOCK_SIZE: u32 = 128 * 1024;
pub const HEADER_SIZE: usize = 24;
/// Maximum decoded index size; prevents corrupt headers from allocating gigabytes.
pub const MAX_INDEX_SIZE: u32 = 64 * 1024 * 1024;
/// Block flag: the block contains an LZ4 frame.
pub const BLOCK_COMPRESSED: u8 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block {
    pub stored_size: u32,
    pub flags: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub path: String,
    pub size: u64,
    pub first_block: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Index {
    pub blocks: Vec<Block>,
    pub entries: Vec<Entry>,
}

fn block_count(size: u64, block_size: u32) -> u64 {
    size.div_ceil(u64::from(block_size))
}

impl Index {
    ///
    /// # Errors
    ///
    /// Returns an error if block counts, entry counts, or path lengths exceed the archive format limits.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        out.extend_from_slice(&u32::try_from(self.blocks.len())?.to_le_bytes());
        for block in &self.blocks {
            out.extend_from_slice(&block.stored_size.to_le_bytes());
            out.push(block.flags);
        }
        out.extend_from_slice(&u32::try_from(self.entries.len())?.to_le_bytes());
        for entry in &self.entries {
            let len = u16::try_from(entry.path.len())
                .with_context(|| format!("path too long: {}", entry.path))?;
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(entry.path.as_bytes());
            out.extend_from_slice(&entry.size.to_le_bytes());
            out.extend_from_slice(&entry.first_block.to_le_bytes());
        }
        Ok(out)
    }

    ///
    /// # Errors
    ///
    /// Returns an error for truncated data, invalid UTF-8 paths, or trailing index bytes.
    pub fn decode(mut data: &[u8]) -> Result<Self> {
        fn take<'a>(data: &mut &'a [u8], n: usize) -> Result<&'a [u8]> {
            ensure!(data.len() >= n, "archive index is truncated");
            let (head, rest) = data.split_at(n);
            *data = rest;
            Ok(head)
        }
        fn u32_at(data: &mut &[u8]) -> Result<u32> {
            Ok(u32::from_le_bytes(take(data, 4)?.try_into()?))
        }
        let block_count = u32_at(&mut data)?;
        let mut blocks = Vec::new();
        for _ in 0..block_count {
            let stored_size = u32_at(&mut data)?;
            let flags = take(&mut data, 1)?[0];
            blocks.push(Block { stored_size, flags });
        }
        let entry_count = u32_at(&mut data)?;
        let mut entries = Vec::new();
        for _ in 0..entry_count {
            let len = u16::from_le_bytes(take(&mut data, 2)?.try_into()?);
            let path = std::str::from_utf8(take(&mut data, usize::from(len))?)
                .context("archive path is not UTF-8")?
                .to_owned();
            let size = u64::from_le_bytes(take(&mut data, 8)?.try_into()?);
            let first_block = u32_at(&mut data)?;
            entries.push(Entry {
                path,
                size,
                first_block,
            });
        }
        ensure!(data.is_empty(), "archive index has trailing data");
        Ok(Self { blocks, entries })
    }
}

#[must_use]
pub fn encode_header(block_size: u32, stored_index: u32, index: u32) -> [u8; HEADER_SIZE] {
    let mut header = [0; HEADER_SIZE];
    header[..8].copy_from_slice(&MAGIC);
    for (i, value) in [VERSION, block_size, stored_index, index]
        .into_iter()
        .enumerate()
    {
        header[8 + i * 4..12 + i * 4].copy_from_slice(&value.to_le_bytes());
    }
    header
}

fn decode_header_field(header: &[u8; HEADER_SIZE], offset: usize) -> Result<u32> {
    let bytes = header
        .get(offset..offset + 4)
        .context("archive header is truncated")?;
    Ok(u32::from_le_bytes(bytes.try_into()?))
}

/// Decode one complete frame into an exactly sized buffer, checking its footer
/// and rejecting trailing bytes or decoded data beyond the archive's size.
///
/// # Errors
///
/// Returns an error if the LZ4 frame is corrupt, truncated, has trailing data, or decodes to a different size.
pub fn decompress_frame(stored: &[u8], size: usize) -> Result<Vec<u8>> {
    let mut decoder = lz4::Decoder::new(stored)?;
    let mut output = vec![0; size];
    decoder.read_exact(&mut output)?;
    // Read through the footer even when the destination filled exactly, while
    // bounding excess decoded data to one byte for corrupt size metadata.
    let mut extra = [0];
    ensure!(
        decoder.read(&mut extra)? == 0,
        "LZ4 frame exceeds the archive size"
    );
    let (remaining, result) = decoder.finish();
    result.context("LZ4 frame is truncated")?;
    ensure!(remaining.is_empty(), "LZ4 frame has trailing data");
    Ok(output)
}

struct BlockInfo {
    offset: u64,
    stored_size: u32,
    compressed: bool,
}

struct EntryInfo {
    size: u64,
    first_block: u32,
}

pub struct Archive {
    path: PathBuf,
    file: Mutex<File>,
    block_size: u32,
    blocks: Vec<BlockInfo>,
    entries: HashMap<String, EntryInfo>,
    paths: Vec<String>,
}

impl Archive {
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or its header, index, paths, or block metadata are invalid.
    pub fn open(path: &Path) -> Result<Self> {
        let mut file =
            File::open(path).with_context(|| format!("cannot open {}", path.display()))?;
        let mut header = [0; HEADER_SIZE];
        file.read_exact(&mut header)
            .context("archive header is truncated")?;
        ensure!(
            header.starts_with(&MAGIC),
            "{} is not a Deflorta archive",
            path.display()
        );
        let version = decode_header_field(&header, 8)?;
        let block_size = decode_header_field(&header, 12)?;
        let stored_index_size = decode_header_field(&header, 16)?;
        let index_size = decode_header_field(&header, 20)?;

        if version != VERSION {
            bail!("unsupported archive version {version} (expected {VERSION})");
        }
        ensure!(
            block_size == BLOCK_SIZE,
            "unsupported archive block size {block_size}"
        );

        let file_size = file.metadata()?.len();
        let data_size = file_size
            .checked_sub(HEADER_SIZE as u64)
            .context("archive header is truncated")?;
        ensure!(
            u64::from(stored_index_size) <= data_size,
            "archive index is truncated"
        );
        ensure!(
            stored_index_size <= MAX_INDEX_SIZE && index_size <= MAX_INDEX_SIZE,
            "archive index is too large"
        );

        let mut stored = vec![0; usize::try_from(stored_index_size)?];
        file.read_exact(&mut stored)
            .context("archive index is truncated")?;
        let index = decompress_frame(&stored, usize::try_from(index_size)?)
            .context("archive index is corrupt")?;
        let index = Index::decode(&index)?;

        let mut offset = u64::try_from(HEADER_SIZE + stored.len())?;
        let mut blocks = Vec::new();
        for block in &index.blocks {
            ensure!(
                block.flags & !BLOCK_COMPRESSED == 0,
                "unsupported archive block flags"
            );
            ensure!(
                block.stored_size > 0 && block.stored_size <= BLOCK_SIZE,
                "invalid archive block size"
            );
            blocks.push(BlockInfo {
                offset,
                stored_size: block.stored_size,
                compressed: block.flags & BLOCK_COMPRESSED != 0,
            });
            offset += u64::from(block.stored_size);
            ensure!(offset <= file_size, "archive block data is truncated");
        }
        let mut entries = HashMap::new();
        let mut paths = Vec::new();
        for entry in index.entries {
            ensure!(
                crate::files::normalize_game_path(Path::new(&entry.path)).as_deref()
                    == Some(entry.path.as_str()),
                "invalid archive path '{}'",
                entry.path
            );
            ensure!(
                !entries.contains_key(&entry.path),
                "duplicate archive path '{}'",
                entry.path
            );
            let end = u64::from(entry.first_block) + block_count(entry.size, block_size);
            ensure!(
                end <= index.blocks.len() as u64,
                "archive entry '{}' points past the last block",
                entry.path
            );
            paths.push(entry.path.clone());
            entries.insert(
                entry.path,
                EntryInfo {
                    size: entry.size,
                    first_block: entry.first_block,
                },
            );
        }
        Ok(Self {
            path: path.to_owned(),
            file: Mutex::new(file),
            block_size,
            blocks,
            entries,
            paths,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn entries(&self) -> impl Iterator<Item = (&str, u64)> {
        self.paths
            .iter()
            .map(|path| (path.as_str(), self.entries[path].size))
    }

    pub fn stored_size(&self, path: &str) -> Option<u64> {
        let entry = self.entries.get(path)?;
        let first = usize::try_from(entry.first_block).ok()?;
        let count = usize::try_from(block_count(entry.size, self.block_size)).ok()?;
        Some(
            self.blocks[first..first + count]
                .iter()
                .map(|b| u64::from(b.stored_size))
                .sum(),
        )
    }

    pub fn contains(&self, path: &str) -> bool {
        self.entries.contains_key(path)
    }

    pub fn size(&self, path: &str) -> Option<u64> {
        self.entries.get(path).map(|e| e.size)
    }

    ///
    /// # Errors
    ///
    /// Returns an error if the entry is missing or its blocks cannot be read or decoded.
    pub fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        let mut reader = Self::open_entry_in(self, path)?;
        let mut data = Vec::with_capacity(usize::try_from(reader.size).unwrap_or(0));
        reader.read_to_end(&mut data)?;
        Ok(data)
    }

    ///
    /// # Errors
    ///
    /// Returns an error if the requested path is absent from the archive.
    pub fn open_entry(self: &Arc<Self>, path: &str) -> io::Result<EntryReader<Arc<Self>>> {
        Self::open_entry_in(self.clone(), path)
    }

    fn open_entry_in<A: AsRef<Self>>(archive: A, path: &str) -> io::Result<EntryReader<A>> {
        let entry = archive.as_ref().entries.get(path).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("'{path}' is not in the archive"),
            )
        })?;
        Ok(EntryReader {
            first_block: entry.first_block,
            size: entry.size,
            archive,
            position: 0,
            block: None,
        })
    }

    fn read_block(&self, index: u32, len: usize) -> io::Result<Vec<u8>> {
        let info = &self.blocks[index as usize];
        let mut stored = vec![0; info.stored_size as usize];
        {
            let mut file = self
                .file
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            file.seek(SeekFrom::Start(info.offset))?;
            file.read_exact(&mut stored)?;
        }
        if !info.compressed {
            if stored.len() != len {
                return Err(corrupt(index));
            }
            return Ok(stored);
        }
        decompress_frame(&stored, len).map_err(|_| corrupt(index))
    }
}

impl AsRef<Self> for Archive {
    fn as_ref(&self) -> &Self {
        self
    }
}

fn corrupt(block: u32) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("archive block {block} is corrupt"),
    )
}

pub struct EntryReader<A: AsRef<Archive>> {
    archive: A,
    first_block: u32,
    size: u64,
    position: u64,
    block: Option<(u64, Vec<u8>)>,
}

impl<A: AsRef<Archive>> EntryReader<A> {
    pub const fn size(&self) -> u64 {
        self.size
    }
}

impl<A: AsRef<Archive>> Read for EntryReader<A> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.position >= self.size || buf.is_empty() {
            return Ok(0);
        }
        let block_size = u64::from(self.archive.as_ref().block_size);
        let block = self.position / block_size;
        if self.block.as_ref().is_none_or(|(b, _)| *b != block) {
            let start = block * block_size;
            let len = usize::try_from(block_size.min(self.size - start))
                .map_err(|_| corrupt(self.first_block))?;
            let index = u32::try_from(u64::from(self.first_block) + block)
                .map_err(|_| corrupt(self.first_block))?;
            let data = self.archive.as_ref().read_block(index, len)?;
            self.block = Some((block, data));
        }
        let (_, data) = self.block.as_ref().expect("block was just loaded");
        let offset = usize::try_from(self.position - block * block_size).unwrap_or(usize::MAX);
        let available = &data[offset.min(data.len())..];
        let n = available.len().min(buf.len());
        buf[..n].copy_from_slice(&available[..n]);
        self.position += n as u64;
        Ok(n)
    }
}

impl<A: AsRef<Archive>> Seek for EntryReader<A> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let target = match pos {
            SeekFrom::Start(offset) => Some(offset),
            SeekFrom::End(delta) => self.size.checked_add_signed(delta),
            SeekFrom::Current(delta) => self.position.checked_add_signed(delta),
        };
        let target = target.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek before the start of the entry",
            )
        })?;
        self.position = target;
        Ok(target)
    }
}
