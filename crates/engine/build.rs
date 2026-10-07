//! Prepares embedded assets: compiles WGSL shaders to SPIR-V and, in release
//! builds, minifies the JavaScript runtime. Outputs land in `OUT_DIR` under the source file names
//! (`shaders/<name>.spv`, `runtime/<name>.js`).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use naga::back::spv;
use naga::valid::{Capabilities, ValidationFlags, Validator};
use oxc::allocator::Allocator;
use oxc::codegen::{Codegen, CodegenOptions, CommentOptions};
use oxc::minifier::{CompressOptions, Minifier, MinifierOptions};
use oxc::parser::Parser;
use oxc::span::SourceType;

const SHADER_DIR: &str = "src/render";
const RUNTIME_DIR: &str = "runtime";

fn main() -> Result<()> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let release = env::var("PROFILE")? == "release";
    build_dir(
        SHADER_DIR,
        "wgsl",
        &out_dir.join("shaders"),
        "spv",
        |path| {
            compile_shader(path).map(|words| words.iter().flat_map(|w| w.to_le_bytes()).collect())
        },
    )?;
    build_dir(RUNTIME_DIR, "js", &out_dir.join("runtime"), "js", |path| {
        let source = fs::read_to_string(path)?;
        let source = deflorta_data::compile_jsx(&path.to_string_lossy(), &source)?;
        if release {
            minify_js(&source).map(String::into_bytes)
        } else {
            Ok(source.into_owned().into_bytes())
        }
    })?;
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

fn minify_js(source: &str) -> Result<String> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::mjs()).parse();
    if !parsed.diagnostics.is_empty() {
        let errors: Vec<_> = parsed.diagnostics.iter().map(ToString::to_string).collect();
        bail!(errors.join("\n"));
    }
    let mut program = parsed.program;
    let minified = Minifier::new(MinifierOptions {
        compress: Some(CompressOptions::smallest()),
        ..MinifierOptions::default()
    })
    .minify(&allocator, &mut program);
    Ok(Codegen::new()
        .with_options(CodegenOptions {
            minify: true,
            comments: CommentOptions::disabled(),
            ..CodegenOptions::default()
        })
        .with_scoping(minified.scoping)
        .build(&program)
        .code)
}
