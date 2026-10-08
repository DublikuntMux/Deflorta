#!/usr/bin/env python3
"""Build a portable Deflorta distribution with external runtimes and templates."""

import argparse
import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ANDROID_TARGETS = {
    "arm64-v8a": "android-aarch64",
    "x86_64": "android-x86_64",
}
ANDROID_API = 26


def capture(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True)


def build(package, release, target):
    command = ["cargo", "build", "--locked", "-p", package, "--message-format=json-render-diagnostics"]
    if release:
        command.append("--release")
        if package == "deflorta-launcher-desktop":
            command.append("--no-default-features")
    if target:
        command.extend(["--target", target])
    result = subprocess.run(
        command, cwd=ROOT, text=True, stdout=subprocess.PIPE, check=True
    )
    executables = [
        artifact["executable"]
        for line in result.stdout.splitlines()
        if (artifact := json.loads(line)).get("reason") == "compiler-artifact"
        and artifact.get("executable")
    ]
    if len(executables) != 1:
        raise RuntimeError(f"expected one executable from {package}, got {executables}")
    return Path(executables[0])


def build_android(abi, release, destination):
    command = [
        "cargo", "ndk", "-t", abi, "--platform", str(ANDROID_API),
        "-o", str(destination), "--link-libcxx-shared",
        "build", "--locked", "-p", "deflorta-launcher-android",
    ]
    if release:
        command.append("--release")
    else:
        command.extend(["--features", "debug-formats"])
    environment = os.environ.copy()
    ndk = environment.get("ANDROID_NDK_HOME") or environment.get("ANDROID_NDK_ROOT")
    if not ndk:
        raise RuntimeError("set ANDROID_NDK_HOME to your installed NDK (r30)")
    # SpiderMonkey's configure/make build uses these names too.
    environment["ANDROID_NDK_HOME"] = ndk
    environment["ANDROID_NDK_ROOT"] = ndk
    environment["ANDROID_API_LEVEL"] = str(ANDROID_API)
    environment["CXXSTDLIB"] = "c++_shared"
    subprocess.run(command, cwd=ROOT, env=environment, check=True)
    library = destination / abi / "libdeflorta.so"
    if not library.is_file():
        raise RuntimeError(f"Android build did not produce {library}")
    # Keep debug info in Cargo output; exports must not require an NDK.
    strip_name = "llvm-strip.exe" if os.name == "nt" else "llvm-strip"
    strip = list(Path(ndk).glob(f"toolchains/llvm/prebuilt/*/bin/{strip_name}"))
    if len(strip) != 1:
        raise RuntimeError(f"cannot locate {strip_name} in {ndk}")
    subprocess.run([str(strip[0]), "--strip-debug", str(library)], check=True)
    # Rust dependencies are statically linked; only libc++ must ship separately.
    for extra in (destination / abi).glob("*.so"):
        if extra.name not in {"libdeflorta.so", "libc++_shared.so"}:
            extra.unlink()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output",
        type=Path,
        default=ROOT / "dist",
        help="distribution folder (default: dist/)",
    )
    parser.add_argument(
        "--target",
        default=os.environ.get("CARGO_BUILD_TARGET"),
        help="Rust target triple (default: native)",
    )
    parser.add_argument(
        "--android", action="store_true",
        help="also build Android export runtimes (requires cargo-ndk and NDK)",
    )
    parser.add_argument(
        "--android-abi", action="append", choices=ANDROID_TARGETS,
        help="ABI to include; repeat for more (default: arm64-v8a)",
    )
    args = parser.parse_args()
    output = args.output.resolve()
    metadata = json.loads(
        capture("cargo", "metadata", "--no-deps", "--format-version", "1")
    )
    cargo_target = Path(metadata["target_directory"]).resolve()
    protected = [
        ROOT / name for name in [".git", ".cargo", "android", "crates", "game", "scripts", "tests"]
    ]
    protected.append(cargo_target)
    if ROOT.is_relative_to(output) or any(
        path.is_relative_to(output) or output.is_relative_to(path) for path in protected
    ):
        parser.error(
            "output must not overlap source folders or Cargo's target directory"
        )
    cfg_command = ["rustc", "--print", "cfg"]
    if args.target:
        cfg_command.extend(["--target", args.target])
    cfg = dict(
        line.split("=", 1) for line in capture(*cfg_command).splitlines() if "=" in line
    )
    platform = f"{json.loads(cfg['target_os'])}-{json.loads(cfg['target_arch'])}"
    if platform.startswith("android-"):
        parser.error("use --android to add Android runtimes to a host distribution")
    if args.android_abi and not args.android:
        parser.error("--android-abi requires --android")
    if args.android:
        ndk = os.environ.get("ANDROID_NDK_HOME") or os.environ.get("ANDROID_NDK_ROOT")
        if not ndk or not Path(ndk).is_dir():
            parser.error("set ANDROID_NDK_HOME to your installed NDK (r30)")
        if not shutil.which("cargo-ndk"):
            parser.error("install cargo-ndk with: cargo install cargo-ndk --locked")

    cli = build("deflorta-cli", True, args.target)
    debug = build("deflorta-launcher-desktop", False, args.target)
    release = build("deflorta-launcher-desktop", True, args.target)
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(
        prefix=".deflorta-dist-", dir=output.parent
    ) as temporary:
        staging = Path(temporary) / "distribution"
        staging.mkdir()
        shutil.copy2(cli, staging / cli.name)
        for profile, launcher in [("debug", debug), ("release", release)]:
            runtime = staging / "target" / platform / profile
            runtime.mkdir(parents=True)
            shutil.copy2(launcher, runtime / launcher.name)
        template = staging / "template" / "game"
        shutil.copytree(ROOT / "crates" / "cli" / "templates", template)
        (template / "gitignore").rename(template / ".gitignore")
        shutil.copytree(ROOT / "game" / "fonts", template / "fonts")
        shutil.copy2(ROOT / "crates" / "js-bridge" / "runtime" / "deflorta.d.ts", template)
        for directory in ["images", "audio", "movies", "tl"]:
            (template / directory).mkdir()
        shutil.copytree(
            ROOT / "crates" / "js-bridge" / "runtime", staging / "template" / "runtime"
        )
        shutil.copytree(
            ROOT / "android", staging / "template" / "android",
            ignore=shutil.ignore_patterns(".gradle", ".kotlin", "build", "local.properties"),
        )
        if args.android:
            for abi in dict.fromkeys(args.android_abi or ["arm64-v8a"]):
                for mode in ["debug", "release"]:
                    destination = staging / "target" / ANDROID_TARGETS[abi] / mode / "jniLibs"
                    build_android(abi, mode == "release", destination)
        if output.exists():
            shutil.rmtree(output)
        staging.rename(output)
    print(f"Built distribution: {output} ({platform})")


if __name__ == "__main__":
    main()
