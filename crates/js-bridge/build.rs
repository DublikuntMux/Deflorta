use anyhow::{Context, Result, bail};
use oxc::allocator::Allocator;
use oxc::codegen::{Codegen, CodegenOptions, CommentOptions};
use oxc::minifier::{CompressOptions, Minifier, MinifierOptions};
use oxc::parser::Parser;
use oxc::span::SourceType;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const RUNTIME_DIR: &str = "runtime";

fn main() -> Result<()> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let release = env::var("PROFILE")? == "release";
    build_dir(RUNTIME_DIR, "js", &out_dir.join("runtime"), "js", |path| {
        let source = fs::read_to_string(path)?;
        let source = deflorta_assets::compile_jsx(&path.to_string_lossy(), &source)?;
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
