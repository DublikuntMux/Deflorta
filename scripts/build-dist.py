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


def capture(*args):
    return subprocess.check_output(args, cwd=ROOT, text=True)


def build(package, release, target):
    command = ["cargo", "build", "--locked", "-p", package, "--message-format=json-render-diagnostics"]
    if release:
        command.append("--release")
        if package == "deflorta-launcher":
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
    args = parser.parse_args()
    output = args.output.resolve()
    metadata = json.loads(
        capture("cargo", "metadata", "--no-deps", "--format-version", "1")
    )
    cargo_target = Path(metadata["target_directory"]).resolve()
    protected = [
        ROOT / name for name in [".git", ".cargo", "crates", "game", "scripts", "tests"]
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

    cli = build("deflorta-cli", True, args.target)
    debug = build("deflorta-launcher", False, args.target)
    release = build("deflorta-launcher", True, args.target)
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
        shutil.copy2(ROOT / "crates" / "engine" / "runtime" / "deflorta.d.ts", template)
        for directory in ["images", "audio", "movies", "tl"]:
            (template / directory).mkdir()
        shutil.copytree(
            ROOT / "crates" / "engine" / "runtime", staging / "template" / "runtime"
        )
        if output.exists():
            shutil.rmtree(output)
        staging.rename(output)
    print(f"Built distribution: {output} ({platform})")


if __name__ == "__main__":
    main()
