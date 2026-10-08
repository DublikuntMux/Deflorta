"""Release checks using isolated Git histories and representative distributions."""

import hashlib
import importlib.util
import shutil
import subprocess
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path

import tomllib

spec = importlib.util.spec_from_file_location(
    "release", Path(__file__).with_name("release.py")
)
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)


class ReleaseTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.git("init", "-q")
        self.git("config", "user.name", "Release Test")
        self.git("config", "user.email", "test@example.invalid")

    def git(self, *args):
        return subprocess.check_output(["git", *args], cwd=self.root, text=True).strip()

    def commit(self, title):
        self.git("commit", "--allow-empty", "-qm", title)
        return self.git("rev-parse", "HEAD")

    def write(self, path, text="fixture"):
        destination = self.root / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(text, encoding="utf-8")
        return destination

    def test_version_titles(self):
        self.assertEqual(release.version("v1.0.0"), "1.0.0")
        for title in (
            "1.0.0",
            "v01.0.0",
            "v1.0.0-beta",
            "release v1.0.0",
            "v1.0.0 extra",
        ):
            with self.assertRaises(ValueError):
                release.version(title)
        self.commit("Ordinary change")
        self.assertEqual(release.prepare(self.root), "")
        self.assertFalse((self.root / "release-notes.md").exists())

    def test_first_release_and_rerun(self):
        self.commit("Initial change")
        self.commit("v1.0.0")
        self.assertEqual(release.prepare(self.root), "v1.0.0")
        original = (self.root / "release-notes.md").read_text()
        self.assertIn("Initial change", original)
        self.assertNotIn("- v1.0.0", original)
        self.git("tag", "v1.0.0")
        release.prepare(self.root)
        self.assertEqual((self.root / "release-notes.md").read_text(), original)

    def test_changelog_range_ignores_unrelated_tags(self):
        self.commit("Old change")
        self.git("tag", "v1.0.0")
        self.git("checkout", "-qb", "unrelated")
        self.commit("Unrelated change")
        self.git("tag", "v9.0.0")
        self.git("checkout", "-")
        change = self.commit("Fix [exports] and `templates`")
        self.commit("v1.1.0")
        release.prepare(self.root)
        notes = (self.root / "release-notes.md").read_text()
        self.assertIn("Fix \\[exports\\] and \\`templates\\`", notes)
        self.assertIn(change, notes)
        self.assertIn("compare/v1.0.0...v1.1.0", notes)
        self.assertNotIn("Old change", notes)
        self.assertNotIn("Unrelated change", notes)

    def test_reusing_tag_at_another_commit_fails(self):
        self.commit("Original release")
        self.git("tag", "v1.0.0")
        self.commit("v1.0.0")
        with self.assertRaisesRegex(ValueError, "another commit"):
            release.prepare(self.root)

    def test_stamp_preserves_dependency_versions_and_lockfile(self):
        shutil.copy2(release.ROOT / "Cargo.toml", self.root / "Cargo.toml")
        shutil.copy2(release.ROOT / "Cargo.lock", self.root / "Cargo.lock")
        workspace = tomllib.loads((self.root / "Cargo.toml").read_text())["workspace"]
        for member in workspace["members"]:
            destination = self.root / member
            destination.mkdir(parents=True)
            shutil.copy2(
                release.ROOT / member / "Cargo.toml", destination / "Cargo.toml"
            )
        before = tomllib.loads((self.root / "Cargo.lock").read_text())["package"]
        release.stamp("v1.2.3", self.root)
        after = tomllib.loads((self.root / "Cargo.lock").read_text())["package"]
        self.assertEqual(
            tomllib.loads((self.root / "Cargo.toml").read_text())["workspace"][
                "package"
            ]["version"],
            "1.2.3",
        )
        self.assertEqual(len(before), len(after))
        for original, stamped in zip(before, after):
            expected = dict(original)
            if "source" not in original:
                expected["version"] = "1.2.3"
            self.assertEqual(stamped, expected)

    def test_archives_include_all_exports_and_preserve_permissions(self):
        artifacts = self.root / "artifacts"
        artifacts.mkdir()
        self.write("LICENSE.md", "engine license")
        for platform in release.DESKTOPS:
            base = f"build-{platform}/dist"
            windows = platform == "windows-x86_64"
            cli = "deflorta.exe" if windows else "deflorta"
            self.write(f"{base}/{cli}").chmod(0o755)
            for profile in ("debug", "release"):
                launcher = "deflorta-launcher.exe" if windows else "deflorta-launcher"
                self.write(f"{base}/target/{platform}/{profile}/{launcher}").chmod(
                    0o755
                )
                if windows:
                    self.write(f"{base}/target/{platform}/{profile}/vcruntime140.dll")
            self.write(f"{base}/template/game/.gitignore")
            self.write(f"{base}/template/game/deflorta.d.ts")
            self.write(f"{base}/template/android/gradlew").chmod(0o755)
            self.write(f"{base}/template/android/gradle/wrapper/gradle-wrapper.jar")
            if not windows:
                for target, abi in release.ANDROID.items():
                    for profile in ("debug", "release"):
                        for library in ("libdeflorta.so", "libc++_shared.so"):
                            self.write(
                                f"{base}/target/{target}/{profile}/jniLibs/{abi}/{library}"
                            )
            with tarfile.open(artifacts / f"{platform}.tar", "w") as archive:
                archive.add(self.root / base, arcname="dist")
        output = self.root / "assets"
        release.package("v1.2.3", artifacts, output, self.root)
        with tarfile.open(output / "deflorta-v1.2.3-linux-x86_64.tar.gz") as archive:
            self.assertEqual(archive.getmember("dist/deflorta").mode & 0o111, 0o111)
            self.assertEqual(
                archive.getmember("dist/template/android/gradlew").mode & 0o111, 0o111
            )
            self.assertIn(
                "dist/target/windows-x86_64/release/vcruntime140.dll",
                archive.getnames(),
            )
            self.assertIn("dist/template/game/.gitignore", archive.getnames())
        with zipfile.ZipFile(output / "deflorta-v1.2.3-windows-x86_64.zip") as archive:
            self.assertIn(
                "dist/target/linux-x86_64/release/deflorta-launcher", archive.namelist()
            )
            self.assertIn(
                "dist/target/android-x86_64/debug/jniLibs/x86_64/libc++_shared.so",
                archive.namelist(),
            )
            self.assertEqual(archive.read("dist/VERSION"), b"v1.2.3\n")
        with tarfile.open(output / "deflorta-v1.2.3-android-export.tar.gz") as archive:
            self.assertNotIn("dist/deflorta", archive.getnames())
            self.assertIn(
                "dist/target/android-aarch64/release/jniLibs/arm64-v8a/libdeflorta.so",
                archive.getnames(),
            )
        checksums = (output / "SHA256SUMS").read_text().splitlines()
        self.assertEqual(len(checksums), 3)
        for line in checksums:
            digest, filename = line.split("  ")
            self.assertEqual(
                digest, hashlib.sha256((output / filename).read_bytes()).hexdigest()
            )
        (
            artifacts
            / "linux-x86_64/dist/target/android-aarch64/debug/jniLibs/arm64-v8a/libc++_shared.so"
        ).unlink()
        with self.assertRaisesRegex(ValueError, "missing"):
            release.validate_distribution(
                artifacts / "linux-x86_64/dist", "linux-x86_64"
            )


if __name__ == "__main__":
    unittest.main()
