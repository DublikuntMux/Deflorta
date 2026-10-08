use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use deflorta_assets::GameFiles;
use oxc::allocator::Allocator;

use crate::graph::{Graph, builtin_exports};

pub const OUTPUT_DIRS: &[&str] = &["build", "dist", "node_modules"];

const SOURCE_EXTENSIONS: &[&str] = &["js", "jsx", "mjs", "cjs", "ts", "mts", "cts", "map"];
const TOOLING_FILES: &[&str] = &[
    "jsconfig.json",
    "tsconfig.json",
    "package.json",
    "package-lock.json",
];

pub struct Project {
    pub dir: PathBuf,
    pub files: GameFiles,
}

impl Project {
    pub fn open(dir: &Path) -> Result<Self> {
        if dir.is_file() {
            bail!(
                "'{}' is a file; expected a game project directory",
                dir.display()
            );
        }
        let files = GameFiles::open(dir)?;
        Ok(Self {
            dir: files.location().to_owned(),
            files,
        })
    }

    pub fn build_dir(&self) -> PathBuf {
        self.dir.join("build")
    }

    pub fn dist_dir(&self) -> PathBuf {
        self.dir.join("dist")
    }

    pub fn assets(&self, excluded: &[&Path]) -> Result<Vec<String>> {
        let excluded = excluded
            .iter()
            .map(|path| absolute_path(path))
            .collect::<Result<Vec<_>>>()?;
        let mut out = Vec::new();
        collect(&self.dir, &self.dir, &excluded, &mut out)?;
        out.sort();
        Ok(out)
    }

    pub fn graph<'a>(&self, allocator: &'a Allocator) -> Graph<'a> {
        let mut graph = Graph::build(allocator, &self.files, &["main.js"]);
        match builtin_exports() {
            Ok(builtins) => graph.link(&builtins),
            Err(err) => graph
                .report
                .error(format!("cannot load the runtime templates: {err:#}")),
        }
        graph
    }
}

/// Resolve existing ancestors as well as `..`, including for outputs which
/// have not been created yet.
pub fn absolute_path(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return Ok(path.canonicalize()?);
    }
    let path = std::path::absolute(path)?;
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            std::path::Component::CurDir => {}
            other => normalized.push(other.as_os_str()),
        }
    }
    if let (Some(parent), Some(name)) = (normalized.parent(), normalized.file_name()) {
        return Ok(absolute_path(parent)?.join(name));
    }
    Ok(normalized)
}

fn is_shipped(name: &str, is_dir: bool, at_root: bool) -> bool {
    if name.starts_with('.') {
        return false;
    }
    if is_dir {
        return !(at_root && OUTPUT_DIRS.contains(&name));
    }
    let extension = Path::new(name).extension().and_then(|e| e.to_str());
    !(TOOLING_FILES.contains(&name)
        || name.ends_with(".d.ts")
        || extension.is_some_and(|e| SOURCE_EXTENSIONS.iter().any(|s| e.eq_ignore_ascii_case(s))))
}

fn collect(root: &Path, dir: &Path, excluded: &[PathBuf], out: &mut Vec<String>) -> Result<()> {
    for entry in std::fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if excluded.iter().any(|output| path.starts_with(output)) {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            bail!("file name is not UTF-8: {}", path.display());
        };
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            bail!("project symlinks are not supported: {}", path.display());
        }
        let is_dir = kind.is_dir();
        if !is_shipped(name, is_dir, dir == root) {
            continue;
        }
        if is_dir {
            collect(root, &path, excluded, out)?;
        } else if kind.is_file() {
            let relative = path.strip_prefix(root)?;
            let archive_path = deflorta_assets::files::normalize_game_path(relative)
                .with_context(|| format!("invalid file name {}", path.display()))?;
            out.push(archive_path);
        }
    }
    Ok(())
}

pub fn sources(graph: &Graph) -> HashMap<String, String> {
    graph
        .modules
        .iter()
        .map(|m| (m.id.clone(), m.source.to_owned()))
        .collect()
}
