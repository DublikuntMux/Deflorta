use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use naga::back::spv;
use naga::valid::{Capabilities, ValidationFlags, Validator};
const SHADER_DIR: &str = "src/render";

fn main() -> Result<()> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    build_dir(
        SHADER_DIR,
        "wgsl",
        &out_dir.join("shaders"),
        "spv",
        |path| {
            compile_shader(path).map(|words| words.iter().flat_map(|w| w.to_le_bytes()).collect())
        },
    )?;
    Ok(())
}

fn build_dir(
    src_dir: &str,
    ext: &str,
    out_dir: &Path,
    out_ext: &str,
    build: impl Fn(&Path) -> Result<Vec<u8>>,
) -> Result<()> {
    println!("cargo::rerun-if-changed={src_dir}");
    fs::create_dir_all(out_dir)?;
    for entry in fs::read_dir(src_dir)? {
        let path = entry?.path();
        if path.extension().is_none_or(|e| e != ext) {
            continue;
        }
        let output = build(&path).with_context(|| format!("building {}", path.display()))?;
        let name = path.with_extension(out_ext);
        let name = name.file_name().context("source file has no name")?;
        fs::write(out_dir.join(name), output)?;
    }
    Ok(())
}

fn compile_shader(path: &Path) -> Result<Vec<u32>> {
    let source = fs::read_to_string(path)?;
    let file = path.to_string_lossy();
    let module = naga::front::wgsl::parse_str(&source)
        .map_err(|e| anyhow!(e.emit_to_string_with_path(&source, &file)))?;
    let info = Validator::new(ValidationFlags::all(), Capabilities::default())
        .validate(&module)
        .map_err(|e| anyhow!(e.emit_to_string_with_path(&source, &file)))?;
    let mut options = spv::Options::default();
    options
        .flags
        .remove(spv::WriterFlags::ADJUST_COORDINATE_SPACE);
    Ok(spv::write_vec(&module, &info, &options, None)?)
}
