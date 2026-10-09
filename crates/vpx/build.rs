use std::path::{Path, PathBuf};
use std::{env, fs};

/// Per-file instruction-set flags, matching libvpx's own Makefile rules.
const GNU_FLAGS: &[(&str, &str)] = &[
    ("_sse2.c", "-msse2"),
    ("_ssse3.c", "-mssse3"),
    ("_sse4.c", "-msse4.1"),
    ("_avx2.c", "-mavx2"),
    ("_neon_dotprod.c", "-march=armv8.2-a+dotprod"),
    ("_neon_i8mm.c", "-march=armv8.2-a+dotprod+i8mm"),
];

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let libvpx = manifest.join("libvpx");
    let configs = manifest.join("config");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=config");
    assert!(
        libvpx.join("vpx/vpx_decoder.h").exists(),
        "libvpx sources are missing; run: git submodule update --init crates/vpx/libvpx"
    );
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let config = configs.join(&arch);
    assert!(
        config.is_dir(),
        "libvpx has no generated configuration for {arch}; see crates/vpx/generate-config.py"
    );
    let sources = fs::read_to_string(config.join("sources.txt")).unwrap();
    let (asm, c): (Vec<&str>, Vec<&str>) = sources
        .lines()
        .partition(|file| Path::new(file).extension().is_some_and(|ext| ext == "asm"));

    let mut objects = Vec::new();
    if !asm.is_empty() {
        objects.extend(
            nasm_rs::Build::new()
                .include(&libvpx)
                .include(&config)
                .files(asm.iter().map(|file| libvpx.join(file)))
                .compile_objects()
                .unwrap_or_else(|error| {
                    panic!("cannot assemble libvpx ({error}); install NASM and put it on PATH")
                }),
        );
    }

    let base = base_build(&libvpx, &config, &configs);
    let compiler = base.get_compiler();
    let gnu = !compiler.is_like_msvc() || compiler.is_like_clang_cl();
    let mut groups: Vec<(Option<&str>, Vec<&str>)> = vec![(None, Vec::new())];
    for file in c {
        let flag = GNU_FLAGS
            .iter()
            .find(|(suffix, _)| file.ends_with(suffix))
            .and_then(|&(suffix, flag)| {
                if gnu {
                    Some(flag)
                } else {
                    // MSVC exposes SSE intrinsics without flags on x86-64.
                    (suffix == "_avx2.c").then_some("/arch:AVX2")
                }
            });
        match groups.iter_mut().find(|(group, _)| *group == flag) {
            Some((_, files)) => files.push(file),
            None => groups.push((flag, vec![file])),
        }
    }
    for (flag, files) in groups {
        let mut build = base.clone();
        build.files(files.iter().map(|file| libvpx.join(file)));
        if let Some(flag) = flag {
            build.flag(flag);
        }
        objects.extend(build.compile_intermediates());
    }
    cc::Build::new().objects(objects).compile("vpx");
}

fn base_build(libvpx: &Path, config: &Path, configs: &Path) -> cc::Build {
    let mut build = cc::Build::new();
    build
        .include(config)
        .include(configs)
        .include(libvpx)
        // Keep asserts out of the decoder even in dev builds; video playback
        // must stay real-time while debugging games.
        .define("NDEBUG", None)
        .opt_level(3)
        .warnings(false);
    if build.get_compiler().is_like_msvc() {
        build.flag("/std:c11");
    } else {
        build.flag("-std=gnu11");
    }
    build
}
