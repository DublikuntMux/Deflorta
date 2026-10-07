//! Read-only access to a game's files.
//!
//! Files come from a project directory during development or from a `.dm`
//! archive in published games. Paths are relative to the game root, use `/`
//! separators and may not escape the root.

use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};

use crate::archive::{Archive, EntryReader};

/// Normalizes a relative path, refusing anything that leaves the game root.
pub fn normalize_game_path(path: &Path) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_str()?),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

enum Source {
    Directory(PathBuf),
    Archive(Arc<Archive>),
}

/// A game's files. Cheap to clone and shareable across threads.
#[derive(Clone)]
pub struct GameFiles(Arc<Source>);

fn not_found(path: &str) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, format!("'{path}' not found"))
}

fn normalize(path: &str) -> io::Result<String> {
    normalize_game_path(Path::new(path)).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("'{path}' is outside the game directory"),
        )
    })
}

impl GameFiles {
    /// Opens a game: a directory or `.dm` archive containing `main.js`.
    pub fn open(path: &Path) -> Result<Self> {
        let files = if path.is_file() {
            Self::archive(path)?
        } else {
            Self::directory(path)?
        };
        if !files.exists("main.js") {
            bail!("'{}' has no main.js", files.location().display());
        }
        Ok(files)
    }

    pub fn directory(path: &Path) -> Result<Self> {
        let path = path
            .canonicalize()
            .with_context(|| format!("game directory '{}' not found", path.display()))?;
        if !path.is_dir() {
            bail!("'{}' is not a directory", path.display());
        }
        Ok(Self(Arc::new(Source::Directory(path))))
    }

    pub fn archive(path: &Path) -> Result<Self> {
        let archive = Archive::open(path)?;
        Ok(Self(Arc::new(Source::Archive(Arc::new(archive)))))
    }

    /// The archive behind these files, if any.
    pub fn as_archive(&self) -> Option<&Archive> {
        match &*self.0 {
            Source::Directory(_) => None,
            Source::Archive(archive) => Some(archive),
        }
    }

    /// The game directory or archive file.
    pub fn location(&self) -> &Path {
        match &*self.0 {
            Source::Directory(dir) => dir,
            Source::Archive(archive) => archive.path(),
        }
    }

    pub fn is_archive(&self) -> bool {
        matches!(&*self.0, Source::Archive(_))
    }

    pub fn exists(&self, path: &str) -> bool {
        let Ok(path) = normalize(path) else {
            return false;
        };
        match &*self.0 {
            Source::Directory(dir) => dir.join(path).is_file(),
            Source::Archive(archive) => archive.contains(&path),
        }
    }

    pub fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        let path = normalize(path)?;
        match &*self.0 {
            Source::Directory(dir) => std::fs::read(dir.join(path)),
            Source::Archive(archive) => archive.read(&path),
        }
    }

    pub fn read_to_string(&self, path: &str) -> io::Result<String> {
        String::from_utf8(self.read(path)?).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, format!("'{path}' is not UTF-8"))
        })
    }

    /// Opens a file for streaming (media, image headers).
    pub fn open_file(&self, path: &str) -> io::Result<GameReader> {
        let path = normalize(path)?;
        let inner = match &*self.0 {
            Source::Directory(dir) => {
                let file = File::open(dir.join(&path))?;
                if !file.metadata()?.is_file() {
                    return Err(not_found(&path));
                }
                Reader::File(file)
            }
            Source::Archive(archive) => Reader::Entry(archive.open_entry(&path)?),
        };
        Ok(GameReader(inner))
    }

    /// All files below `dir` (recursively), as game paths.
    pub fn list(&self, dir: &str) -> Vec<String> {
        let Ok(dir) = normalize(dir) else {
            return Vec::new();
        };
        let mut files = Vec::new();
        match &*self.0 {
            Source::Directory(root) => collect_files(root, &root.join(&dir), &mut files),
            Source::Archive(archive) => {
                let prefix = format!("{dir}/");
                files.extend(
                    archive
                        .entries()
                        .filter(|(path, _)| path.starts_with(&prefix))
                        .map(|(path, _)| path.to_owned()),
                );
            }
        }
        files.sort();
        files
    }
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, out);
        } else if let Some(name) = path.strip_prefix(root).ok().and_then(normalize_game_path) {
            out.push(name);
        }
    }
}

enum Reader {
    File(File),
    Entry(EntryReader<Arc<Archive>>),
}

/// A seekable game file.
pub struct GameReader(Reader);

impl GameReader {
    pub fn size(&self) -> io::Result<u64> {
        match &self.0 {
            Reader::File(file) => Ok(file.metadata()?.len()),
            Reader::Entry(entry) => Ok(entry.size()),
        }
    }

    pub fn buffered(self) -> BufReader<Self> {
        BufReader::new(self)
    }
}

impl Read for GameReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.0 {
            Reader::File(file) => file.read(buf),
            Reader::Entry(entry) => entry.read(buf),
        }
    }
}

impl Seek for GameReader {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match &mut self.0 {
            Reader::File(file) => file.seek(pos),
            Reader::Entry(entry) => entry.seek(pos),
        }
    }
}

impl symphonia_core::io::MediaSource for GameReader {
    fn is_seekable(&self) -> bool {
        true
    }

    fn byte_len(&self) -> Option<u64> {
        self.size().ok()
    }
}
