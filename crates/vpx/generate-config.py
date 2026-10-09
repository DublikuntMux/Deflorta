#!/usr/bin/env python3
"""Regenerate the checked-in libvpx VP9 decoder configuration.

Run after moving the libvpx submodule to a new release. Requires bash, make,
perl, nasm, and an Android NDK (ANDROID_NDK_HOME) for the AArch64 probes.
"""

import os
import shutil
import subprocess
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
LIBVPX = HERE / "libvpx"
CONFIG = HERE / "config"
OPTIONS = [
    "--disable-vp8",
    "--disable-vp9-encoder",
    "--enable-vp9-decoder",
    "--disable-vp9-highbitdepth",
    "--disable-postproc",
    "--disable-vp9-postproc",
    "--disable-examples",
    "--disable-tools",
    "--disable-docs",
    "--disable-install-docs",
    "--disable-unit-tests",
    "--disable-webm-io",
    "--disable-libyuv",
    "--enable-runtime-cpu-detect",
    "--enable-multithread",
    "--enable-pic",
]
RTCD = ["vp9_rtcd.h", "vpx_dsp_rtcd.h", "vpx_scale_rtcd.h"]
# The probes run for one OS per architecture; these settings differ by OS and
# compiler, so the build decides them instead.
PORTABLE = """#ifdef _MSC_VER
#define INLINE      __inline
#define CONFIG_GCC 0
#define CONFIG_MSVS 1
#else
#define INLINE      inline
#define CONFIG_GCC 1
#define CONFIG_MSVS 0
#endif
#ifdef _WIN32
#define HAVE_PTHREAD_H 0
#define HAVE_UNISTD_H 0
#else
#define HAVE_PTHREAD_H 1
#define HAVE_PTHREAD_SETNAME_NP 1
#define HAVE_UNISTD_H 1
#endif
"""
PORTABLE_NAMES = {
    "INLINE",
    "CONFIG_GCC",
    "CONFIG_MSVS",
    "HAVE_PTHREAD_H",
    "HAVE_PTHREAD_SETNAME_NP",
    "HAVE_UNISTD_H",
}


def ndk_tools():
    ndk = os.environ.get("ANDROID_NDK_HOME")
    if not ndk:
        raise SystemExit("set ANDROID_NDK_HOME to probe AArch64 features")
    bin_dir = next(Path(ndk, "toolchains", "llvm", "prebuilt").iterdir()) / "bin"
    clang = str(bin_dir / "aarch64-linux-android26-clang")
    return {
        "CC": clang,
        "CXX": clang + "++",
        "LD": clang,
        "AS": clang,
        "AR": str(bin_dir / "llvm-ar"),
        "NM": str(bin_dir / "llvm-nm"),
        "STRIP": str(bin_dir / "llvm-strip"),
    }


TARGETS = {
    # AVX-512 has no decoder kernels and MSVC cannot build libvpx's AVX-512 code.
    "x86_64": ("x86_64-linux-gcc", ["--as=nasm", "--disable-avx512"], dict),
    "aarch64": ("arm64-android-gcc", [], ndk_tools),
}


def portable_config(text):
    lines = []
    inserted = False
    for line in text.splitlines():
        parts = line.split()
        if len(parts) >= 2 and parts[0] == "#define" and parts[1] in PORTABLE_NAMES:
            if not inserted:
                lines.append(PORTABLE.rstrip())
                inserted = True
            continue
        lines.append(line)
    if not inserted:
        raise SystemExit("vpx_config.h layout changed; update PORTABLE_NAMES")
    return "\n".join(lines) + "\n"


def sources(listing):
    keep = []
    for line in listing.splitlines():
        if not line.endswith((".c", ".asm")):
            continue
        # vpx_config.c only reports configure flags, and x86_abi_support.asm is
        # included by other assembly files rather than assembled on its own.
        if line in ("vpx_config.c", "vpx_ports/x86_abi_support.asm"):
            continue
        keep.append(line)
    return "\n".join(sorted(keep)) + "\n"


def generate(arch, target, extra, tools):
    with tempfile.TemporaryDirectory() as build:
        env = {**os.environ, **tools()}
        subprocess.run(
            ["bash", str(LIBVPX / "configure"), f"--target={target}", *OPTIONS, *extra],
            cwd=build,
            env=env,
            check=True,
            stdout=subprocess.DEVNULL,
        )
        subprocess.run(
            ["make", "libvpx_srcs.txt", "target=libs"],
            cwd=build,
            env=env,
            check=True,
            stdout=subprocess.DEVNULL,
        )
        build = Path(build)
        out = CONFIG / arch
        shutil.rmtree(out, ignore_errors=True)
        out.mkdir(parents=True)
        config = (build / "vpx_config.h").read_text()
        (out / "vpx_config.h").write_text(portable_config(config))
        for name in RTCD:
            shutil.copy(build / name, out / name)
        if (build / "vpx_config.asm").exists():
            shutil.copy(build / "vpx_config.asm", out / "vpx_config.asm")
        (out / "sources.txt").write_text(sources((build / "libvpx_srcs.txt").read_text()))
        shutil.copy(build / "vpx_version.h", CONFIG / "vpx_version.h")


def main():
    if not (LIBVPX / "configure").exists():
        raise SystemExit("libvpx is missing; run: git submodule update --init crates/vpx/libvpx")
    for arch, (target, extra, tools) in TARGETS.items():
        generate(arch, target, extra, tools)
        print(f"generated {arch} from {target}")


if __name__ == "__main__":
    main()
