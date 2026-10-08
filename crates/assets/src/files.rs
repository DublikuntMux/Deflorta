use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result, bail};

use crate::archive::{Archive, EntryReader};

#[must_use]
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

/// Resolves a normalized game path without traversing project symlinks.
/// The root is canonicalized once when the project is opened.
fn directory_path(root: &Path, path: &str) -> io::Result<PathBuf> {
    let mut resolved = root.to_owned();
    for component in Path::new(path).components() {
        resolved.push(component);
        if std::fs::symlink_metadata(&resolved)?
            .file_type()
            .is_symlink()
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("project symlinks are not supported: {}", resolved.display()),
            ));
        }
    }
    Ok(resolved)
}

impl GameFiles {
    /// Opens a game: a directory or `.dm` archive containing `main.js`.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory or archive cannot be opened or does not contain `main.js`.
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

    ///
    /// # Errors
    ///
    /// Returns an error if the path cannot be canonicalized or does not name a directory.
    pub fn directory(path: &Path) -> Result<Self> {
        let path = path
            .canonicalize()
            .with_context(|| format!("game directory '{}' not found", path.display()))?;
        if !path.is_dir() {
            bail!("'{}' is not a directory", path.display());
        }
        Ok(Self(Arc::new(Source::Directory(path))))
    }

    ///
    /// # Errors
    ///
    /// Returns an error if the archive cannot be read or its format is invalid.
    pub fn archive(path: &Path) -> Result<Self> {
        let archive = Archive::open(path)?;
        Ok(Self(Arc::new(Source::Archive(Arc::new(archive)))))
    }

    #[must_use]
    pub fn as_archive(&self) -> Option<&Archive> {
        match &*self.0 {
            Source::Directory(_) => None,
            Source::Archive(archive) => Some(archive),
        }
    }

    #[must_use]
    pub fn location(&self) -> &Path {
        match &*self.0 {
            Source::Directory(dir) => dir,
            Source::Archive(archive) => archive.path(),
        }
    }

    #[must_use]
    pub fn is_archive(&self) -> bool {
        matches!(&*self.0, Source::Archive(_))
    }

    #[must_use]
    pub fn exists(&self, path: &str) -> bool {
        let Ok(path) = normalize(path) else {
            return false;
        };
        match &*self.0 {
            Source::Directory(dir) => directory_path(dir, &path).is_ok_and(|p| p.is_file()),
            Source::Archive(archive) => archive.contains(&path),
        }
    }

    ///
    /// # Errors
    ///
    /// Returns an error for paths outside the game, project symlinks, missing files, or read/decompression failures.
    pub fn read(&self, path: &str) -> io::Result<Vec<u8>> {
        let path = normalize(path)?;
        match &*self.0 {
            Source::Directory(_) => {
                let mut bytes = Vec::new();
                self.open_file(&path)?.read_to_end(&mut bytes)?;
                Ok(bytes)
            }
            Source::Archive(archive) => archive.read(&path),
        }
    }

    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or its contents are not UTF-8.
    pub fn read_to_string(&self, path: &str) -> io::Result<String> {
        String::from_utf8(self.read(path)?).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidData, format!("'{path}' is not UTF-8"))
        })
    }

    ///
    /// # Errors
    ///
    /// Returns an error for paths outside the game, project symlinks, missing entries, or filesystem access failures.
    pub fn open_file(&self, path: &str) -> io::Result<GameReader> {
        let path = normalize(path)?;
        let inner = match &*self.0 {
            Source::Directory(dir) => {
                let file = File::open(directory_path(dir, &path)?)?;
                if !file.metadata()?.is_file() {
                    return Err(not_found(&path));
                }
                Reader::File(file)
            }
            Source::Archive(archive) => Reader::Entry(archive.open_entry(&path)?),
        };
        Ok(GameReader(inner))
    }

    #[must_use]
    pub fn list(&self, dir: &str) -> Vec<String> {
        let Ok(dir) = normalize(dir) else {
            return Vec::new();
        };
        let mut files = Vec::new();
        match &*self.0 {
            Source::Directory(root) => {
                if let Ok(path) = directory_path(root, &dir) {
                    collect_files(root, &path, &mut files);
                }
            }
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
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            collect_files(root, &path, out);
        } else if kind.is_file()
            && let Some(name) = path.strip_prefix(root).ok().and_then(normalize_game_path)
        {
            out.push(name);
        }
    }
}

enum Reader {
    File(File),
    Entry(EntryReader<Arc<Archive>>),
}

pub struct GameReader(Reader);

impl GameReader {
    ///
    /// # Errors
    ///
    /// Returns an error if filesystem metadata for a directory-backed reader cannot be obtained.
    pub fn size(&self) -> io::Result<u64> {
        match &self.0 {
            Reader::File(file) => Ok(file.metadata()?.len()),
            Reader::Entry(entry) => Ok(entry.size()),
        }
    }

    #[must_use]
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn directory_access_rejects_file_directory_and_cycle_symlinks() {
        let temp = std::env::temp_dir().join(format!("deflorta-files-{}", std::process::id()));
        let root = temp.join("game");
        std::fs::create_dir_all(root.join("assets")).unwrap();
        std::fs::create_dir_all(temp.join("outside")).unwrap();
        std::fs::write(root.join("assets/inside.txt"), "inside").unwrap();
        std::fs::write(temp.join("outside/sentinel.txt"), "outside").unwrap();
        symlink(
            temp.join("outside/sentinel.txt"),
            root.join("assets/file.txt"),
        )
        .unwrap();
        symlink(temp.join("outside"), root.join("assets/directory")).unwrap();
        symlink(root.join("assets"), root.join("assets/cycle")).unwrap();
        symlink(
            root.join("assets/inside.txt"),
            root.join("assets/internal.txt"),
        )
        .unwrap();
        let files = GameFiles::directory(&root).unwrap();
        assert_eq!(files.read_to_string("assets/inside.txt").unwrap(), "inside");
        for path in [
            "assets/file.txt",
            "assets/directory/sentinel.txt",
            "assets/internal.txt",
            "assets/cycle/inside.txt",
        ] {
            assert!(!files.exists(path), "{path}");
            assert_eq!(
                files.read(path).unwrap_err().kind(),
                io::ErrorKind::PermissionDenied
            );
            assert!(files.open_file(path).is_err());
        }
        assert_eq!(files.list("assets"), ["assets/inside.txt"]);
        assert_eq!(files.list("assets/directory"), [] as [String; 0]);
        assert!(files.read("../outside/sentinel.txt").is_err());
        std::fs::remove_dir_all(temp).unwrap();
    }
}
