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
    Directory {
        root: PathBuf,
        scripts: Option<PathBuf>,
    },
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

fn source_path(root: &Path, scripts: Option<&Path>, path: &str) -> io::Result<PathBuf> {
    if let Some(scripts) = scripts
        && scripts.join(path).try_exists()?
    {
        return directory_path(scripts, path);
    }
    directory_path(root, path)
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
        Ok(Self(Arc::new(Source::Directory {
            root: path,
            scripts: None,
        })))
    }

    /// Reads compiled scripts from a separate directory while keeping source assets.
    ///
    /// # Errors
    ///
    /// Returns an error if either directory cannot be opened.
    pub fn with_scripts(root: &Path, scripts: &Path) -> Result<Self> {
        let files = Self::directory(root)?;
        let scripts = scripts.canonicalize()?;
        if !scripts.is_dir() {
            bail!("'{}' is not a script directory", scripts.display());
        }
        Ok(Self(Arc::new(Source::Directory {
            root: files.location().to_owned(),
            scripts: Some(scripts),
        })))
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
            Source::Directory { .. } => None,
            Source::Archive(archive) => Some(archive),
        }
    }

    #[must_use]
    pub fn location(&self) -> &Path {
        match &*self.0 {
            Source::Directory { root, .. } => root,
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
            Source::Directory { root, scripts } => {
                source_path(root, scripts.as_deref(), &path).is_ok_and(|p| p.is_file())
            }
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
            Source::Directory { .. } => {
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
            Source::Directory { root, scripts } => {
                let file = File::open(source_path(root, scripts.as_deref(), &path)?)?;
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
            Source::Directory { root, .. } => {
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
    fn compiled_scripts_keep_source_assets_and_reject_symlinks() {
        let root = tempfile::tempdir().unwrap();
        let scripts = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("main.js"), "const view = <View />;").unwrap();
        std::fs::write(root.path().join("asset.txt"), "source asset").unwrap();
        std::fs::write(
            scripts.path().join("main.js"),
            "const view = jsx(View, {});",
        )
        .unwrap();
        symlink(
            root.path().join("asset.txt"),
            scripts.path().join("link.js"),
        )
        .unwrap();
        let files = GameFiles::with_scripts(root.path(), scripts.path()).unwrap();
        assert_eq!(files.location(), root.path());
        assert_eq!(
            files.read_to_string("main.js").unwrap(),
            "const view = jsx(View, {});"
        );
        assert_eq!(files.read_to_string("asset.txt").unwrap(), "source asset");
        assert_eq!(
            std::fs::read_to_string(root.path().join("main.js")).unwrap(),
            "const view = <View />;"
        );
        let mut reader = files.open_file("main.js").unwrap();
        reader.seek(SeekFrom::Start(13)).unwrap();
        let mut tail = String::new();
        reader.read_to_string(&mut tail).unwrap();
        assert_eq!(tail, "jsx(View, {});");
        assert!(!files.exists("link.js"));
        assert_eq!(
            files.read("link.js").unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );
        assert!(files.read("../main.js").is_err());
        assert!(GameFiles::with_scripts(root.path(), &root.path().join("asset.txt")).is_err());
    }

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
