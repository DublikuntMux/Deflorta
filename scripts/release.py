#!/usr/bin/env python3
"""Prepare version-commit releases and assemble their platform distributions."""

import argparse
import hashlib
import os
import re
import shutil
import subprocess
import tarfile
from pathlib import Path

import tomllib

VERSION = re.compile(r"v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)")
DESKTOPS = ("linux-x86_64", "windows-x86_64")
ANDROID = {"android-aarch64": "arm64-v8a", "android-x86_64": "x86_64"}
ROOT = Path(__file__).resolve().parent.parent


def git(*args, root=ROOT):
    return subprocess.check_output(["git", *args], cwd=root, text=True).strip()


def version(tag):
    if not VERSION.fullmatch(tag):
        raise ValueError(
            "release title must be exactly vMAJOR.MINOR.PATCH (for example v1.0.0)"
        )
    return tag[1:]


def prepare(root=ROOT):
    tag = git("log", "-1", "--format=%s", root=root)
    if not VERSION.fullmatch(tag):
        print("Latest commit is not a release commit; skipping release.")
        return ""
    tags = git("tag", "--list", root=root).splitlines()
    if tag in tags and git("rev-list", "-n", "1", tag, root=root) != git(
        "rev-parse", "HEAD", root=root
    ):
        raise ValueError(f"{tag} already points to another commit")
    # Find the closest version tag in this commit's ancestry, excluding our
    # own tag so rerunning a successful release produces the same changelog.
    previous = ""
    if git("rev-list", "--count", "HEAD", root=root) != "1":
        ancestors = git("rev-list", "HEAD^", root=root).splitlines()
        versions = {}
        for candidate in tags:
            if VERSION.fullmatch(candidate) and candidate != tag:
                commit = git("rev-list", "-n", "1", candidate, root=root)
                versions.setdefault(commit, []).append(candidate)
        for commit in ancestors:
            if commit in versions:
                previous = max(
                    versions[commit],
                    key=lambda item: tuple(map(int, item[1:].split("."))),
                )
                break
    revision = f"{previous}..HEAD" if previous else "HEAD"
    entries = git(
        "log", "--reverse", "--format=%H%x09%s", revision, root=root
    ).splitlines()
    repository = os.environ.get("GITHUB_REPOSITORY", "DublikuntMux/Deflorta")
    base = f"https://github.com/{repository}"
    lines = [f"# {tag}", "", "## Changes", ""]
    for entry in entries:
        commit, subject = entry.split("\t", 1)
        if VERSION.fullmatch(subject):
            continue
        subject = re.sub(r"([\\`*_{}\[\]<>])", r"\\\1", subject)
        lines.append(f"- {subject} ([{commit[:7]}]({base}/commit/{commit}))")
    if len(lines) == 4:
        lines.append("- No additional changes since the previous release.")
    if previous:
        lines.extend(["", f"[Full changelog]({base}/compare/{previous}...{tag})"])
    lines.extend(
        [
            "",
            "## Downloads",
            "",
            "Linux and Windows distributions include debug and release exports for Linux x86_64, ",
            "Windows x86_64, Android arm64-v8a, and Android x86_64. Keep each distribution together.",
            "",
            "The Android export archive contains native runtimes and the Gradle template; ",
            "merge it into an existing engine distribution. APK/AAB exports require Java 17 ",
            "and Android SDK 36; release game exports require your own signing key.",
            "",
            "Verify archive downloads against `SHA256SUMS`.",
            "",
        ]
    )
    (root / "release-notes.md").write_text("\n".join(lines), encoding="utf-8")
    return tag


def stamp(tag, root=ROOT):
    release_version = version(tag)
    manifest = root / "Cargo.toml"
    text = manifest.read_text(encoding="utf-8")
    workspace = tomllib.loads(text)["workspace"]
    text, count = re.subn(
        r'(\[workspace\.package\]\s*\nversion\s*=\s*)"[^"]+"',
        lambda match: f'{match[1]}"{release_version}"',
        text,
    )
    if count != 1:
        raise ValueError("cannot locate workspace package version")
    names = {
        tomllib.loads((root / member / "Cargo.toml").read_text(encoding="utf-8"))[
            "package"
        ]["name"]
        for member in workspace["members"]
    }
    lock = root / "Cargo.lock"
    blocks = lock.read_text(encoding="utf-8").split("[[package]]")
    for index, block in enumerate(blocks[1:], 1):
        package = tomllib.loads(block)
        if package["name"] in names and "source" not in package:
            blocks[index] = re.sub(
                r'^version = "[^"]+"',
                f'version = "{release_version}"',
                block,
                count=1,
                flags=re.MULTILINE,
            )
    manifest.write_text(text, encoding="utf-8")
    lock.write_text("[[package]]".join(blocks), encoding="utf-8")


def validate_distribution(path, host):
    required = [
        "template/game/.gitignore",
        "template/game/deflorta.d.ts",
        "template/android/gradlew",
        "template/android/gradle/wrapper/gradle-wrapper.jar",
    ]
    if host:
        required.append("deflorta.exe" if host == "windows-x86_64" else "deflorta")
        for platform in DESKTOPS:
            executable = (
                "deflorta-launcher.exe"
                if platform == "windows-x86_64"
                else "deflorta-launcher"
            )
            for profile in ("debug", "release"):
                required.append(f"target/{platform}/{profile}/{executable}")
    for platform, abi in ANDROID.items():
        for profile in ("debug", "release"):
            for library in ("libdeflorta_android.so", "libc++_shared.so"):
                required.append(f"target/{platform}/{profile}/jniLibs/{abi}/{library}")
    for name in required:
        if not (path / name).is_file():
            raise ValueError(f"distribution is missing {path / name}")


def package(tag, artifacts, output, root=ROOT):
    version(tag)
    distributions = {}
    for platform in DESKTOPS:
        destination = artifacts / platform
        destination.mkdir(parents=True, exist_ok=True)
        with tarfile.open(artifacts / f"{platform}.tar") as archive:
            archive.extractall(destination, filter="data")
        distributions[platform] = destination / "dist"
    linux = distributions["linux-x86_64"]
    windows = distributions["windows-x86_64"]
    shutil.copytree(windows / "target/windows-x86_64", linux / "target/windows-x86_64")
    for platform in ("linux-x86_64", *ANDROID):
        shutil.copytree(linux / "target" / platform, windows / "target" / platform)
    # Use the Unix template so gradlew keeps its executable bit on both hosts.
    shutil.rmtree(windows / "template/android")
    shutil.copytree(linux / "template/android", windows / "template/android")
    output.mkdir(parents=True, exist_ok=True)
    for platform, distribution in distributions.items():
        validate_distribution(distribution, platform)
        shutil.copy2(root / "LICENSE.md", distribution / "LICENSE.md")
        (distribution / "VERSION").write_text(tag + "\n", encoding="utf-8")
        filename = f"deflorta-{tag}-{platform}"
        archive_format = "zip" if platform == "windows-x86_64" else "gztar"
        shutil.make_archive(
            str(output / filename), archive_format, distribution.parent, "dist"
        )
    android = artifacts / "android-export" / "dist"
    for platform in ANDROID:
        shutil.copytree(linux / "target" / platform, android / "target" / platform)
    shutil.copytree(linux / "template/android", android / "template/android")
    shutil.copy2(root / "LICENSE.md", android / "LICENSE.md")
    shutil.copy2(linux / "VERSION", android / "VERSION")
    shutil.make_archive(
        str(output / f"deflorta-{tag}-android-export"), "gztar", android.parent, "dist"
    )
    checksums = []
    for archive in sorted(output.iterdir()):
        if archive.name != "SHA256SUMS":
            with archive.open("rb") as stream:
                digest = hashlib.file_digest(stream, "sha256").hexdigest()
            checksums.append(f"{digest}  {archive.name}\n")
    (output / "SHA256SUMS").write_text("".join(checksums), encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("prepare")
    stamping = commands.add_parser("stamp")
    stamping.add_argument("tag")
    packaging = commands.add_parser("package")
    packaging.add_argument("tag")
    packaging.add_argument("artifacts", type=Path)
    packaging.add_argument("output", type=Path)
    args = parser.parse_args()
    if args.command == "prepare":
        tag = prepare()
        if output := os.environ.get("GITHUB_OUTPUT"):
            with open(output, "a", encoding="utf-8") as stream:
                stream.write(f"tag={tag}\n")
    elif args.command == "stamp":
        stamp(args.tag)
    else:
        package(args.tag, args.artifacts, args.output)


if __name__ == "__main__":
    main()
