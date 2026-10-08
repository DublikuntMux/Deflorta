use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use deflorta_assets::GameFiles;

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Project(PathBuf);

impl Project {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "deflorta-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let project = Self(path);
        success(&cli(["create", project.path(), "--title", "Test Game"]));
        project
    }

    fn path(&self) -> &str {
        self.0.to_str().unwrap()
    }

    fn write(&self, name: &str, text: &str) {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn publish(&self, output: &str) -> Output {
        cli([
            "publish",
            self.path(),
            "-o",
            self.0.join(output).to_str().unwrap(),
        ])
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn cli<const N: usize>(args: [&str; N]) -> Output {
    Command::new(distribution().join(format!("deflorta{}", std::env::consts::EXE_SUFFIX)))
        .args(args)
        // Avoid developer preferences/saves affecting a fresh project's boot.
        .env(
            "XDG_DATA_HOME",
            std::env::temp_dir().join("deflorta-cli-test-data"),
        )
        .output()
        .unwrap()
}

/// Assemble once from real binaries and external template data. Cargo builds
/// only the CLI binary for this test, so build its external launcher explicitly.
fn distribution() -> &'static Path {
    static DISTRIBUTION: OnceLock<PathBuf> = OnceLock::new();
    DISTRIBUTION.get_or_init(|| {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let cli = Path::new(env!("CARGO_BIN_EXE_deflorta"));
        let profile = cli.parent().unwrap();
        let mut build = Command::new(env!("CARGO"));
        build
            .current_dir(&root)
            .args(["build", "--locked", "-p", "deflorta-launcher-desktop"]);
        if profile.file_name().unwrap() == "release" {
            build.arg("--release");
        }
        success(&build.output().unwrap());
        let path = profile.join(format!("test-distribution-{}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        std::fs::copy(cli, path.join(cli.file_name().unwrap())).unwrap();
        let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
        let launcher = profile.join(format!("deflorta-launcher{}", std::env::consts::EXE_SUFFIX));
        for mode in ["debug", "release"] {
            let runtime = path.join("target").join(&platform).join(mode);
            std::fs::create_dir_all(runtime.join("resources")).unwrap();
            std::fs::copy(&launcher, runtime.join(launcher.file_name().unwrap())).unwrap();
            std::fs::write(runtime.join("resources/runtime.txt"), mode).unwrap();
        }
        let template = path.join("template/game");
        copy_tree(&root.join("crates/cli/templates"), &template);
        std::fs::rename(template.join("gitignore"), template.join(".gitignore")).unwrap();
        copy_tree(&root.join("game/fonts"), &template.join("fonts"));
        std::fs::copy(
            root.join("crates/js-bridge/runtime/deflorta.d.ts"),
            template.join("deflorta.d.ts"),
        )
        .unwrap();
        for dir in ["images", "audio", "movies", "tl"] {
            std::fs::create_dir_all(template.join(dir)).unwrap();
        }
        copy_tree(
            &root.join("crates/js-bridge/runtime"),
            &path.join("template/runtime"),
        );
        let android = path.join("template/android");
        std::fs::create_dir_all(android.join("app")).unwrap();
        for file in [
            "settings.gradle.kts",
            "build.gradle.kts",
            "gradle.properties",
            "gradlew",
            "gradlew.bat",
            "app/build.gradle.kts",
            "app/proguard-rules.pro",
        ] {
            std::fs::copy(root.join("android").join(file), android.join(file)).unwrap();
        }
        copy_tree(&root.join("android/gradle"), &android.join("gradle"));
        copy_tree(&root.join("android/app/src"), &android.join("app/src"));
        path
    })
}

fn copy_tree(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in source.read_dir().unwrap() {
        let entry = entry.unwrap();
        let to = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &to);
        } else {
            std::fs::copy(entry.path(), to).unwrap();
        }
    }
}

fn success(output: &Output) -> String {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout.clone()).unwrap()
}

#[test]
fn launcher_runtime_dependencies_exclude_script_compilers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (package, target) in [
        ("deflorta-launcher-desktop", None),
        ("deflorta-launcher-android", Some("aarch64-linux-android")),
        ("deflorta-launcher-android", Some("x86_64-linux-android")),
    ] {
        let mut command = Command::new(env!("CARGO"));
        command.current_dir(&root).args([
            "tree", "--locked", "-p", package, "--edges", "normal", "--prefix", "none",
        ]);
        if let Some(target) = target {
            command.args(["--target", target]);
        }
        let tree = success(&command.output().unwrap());
        assert!(
            !tree
                .lines()
                .any(|line| line.starts_with("oxc") || line.starts_with("deflorta-script-build")),
            "{package} must keep script compilation in build dependencies:\n{tree}"
        );
    }
}

#[test]
fn bundle_and_publish_transcode_media_without_changing_asset_paths_or_sources() {
    let project = Project::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let assets = [
        ("images/portrait.PNG", "game/images/eileen/blush.png"),
        ("movies/intro.mp4", "game/movies/intro.mp4"),
        ("audio/chime.wav", "game/audio/chime.wav"),
    ];
    for (path, source) in assets {
        std::fs::copy(root.join(source), project.0.join(path)).unwrap();
    }
    let source = image::open(project.0.join("images/portrait.PNG"))
        .unwrap()
        .into_rgba8();
    source.save(project.0.join("images/portrait.webp")).unwrap();
    for (path, cover) in [("audio/song.mp4", false), ("audio/album.m4a", true)] {
        let mut encode = Command::new("ffmpeg");
        encode
            .args(["-nostdin", "-hide_banner", "-loglevel", "error", "-i"])
            .arg(project.0.join("audio/chime.wav"));
        if cover {
            encode
                .arg("-i")
                .arg(project.0.join("images/portrait.PNG"))
                .args([
                    "-map",
                    "0:a:0",
                    "-map",
                    "1:v:0",
                    "-c:v",
                    "copy",
                    "-disposition:v",
                    "attached_pic",
                ]);
        }
        encode.args(["-c:a", "aac"]).arg(project.0.join(path));
        success(&encode.output().unwrap());
    }
    let paths: Vec<_> = assets
        .iter()
        .map(|(path, _)| *path)
        .chain(["images/portrait.webp", "audio/song.mp4", "audio/album.m4a"])
        .collect();
    let originals: Vec<_> = paths
        .iter()
        .map(|path| std::fs::read(project.0.join(path)).unwrap())
        .collect();
    for mode in ["bundle", "publish"] {
        let output = if mode == "bundle" {
            cli(["bundle", project.path()])
        } else {
            project.publish("dist/test")
        };
        success(&output);
        let archive = project.0.join(if mode == "bundle" {
            "build/game.dm"
        } else {
            "dist/test/game.dm"
        });
        let files = GameFiles::open(&archive).unwrap();
        for path in ["images/portrait.PNG", "images/portrait.webp"] {
            let bytes = files.read(path).unwrap();
            assert_eq!(&bytes[..4], b"RIFF");
            assert_eq!(&bytes[8..12], b"WEBP");
            let decoded = image::load_from_memory(&bytes).unwrap().into_rgba8();
            assert_eq!(decoded.dimensions(), source.dimensions());
            for (actual, expected) in decoded.pixels().zip(source.pixels()) {
                assert_eq!(actual[3], expected[3]);
                if expected[3] > 0 {
                    assert_eq!(actual, expected);
                }
            }
        }
        for path in ["audio/chime.wav", "audio/song.mp4", "audio/album.m4a"] {
            let bytes = files.read(path).unwrap();
            assert!(bytes.starts_with(b"OggS"), "{path}");
            assert!(
                bytes.windows(7).any(|header| header == b"\x01vorbis"),
                "{path}"
            );
        }
        assert!(
            files
                .read("movies/intro.mp4")
                .unwrap()
                .starts_with(&[0x1a, 0x45, 0xdf, 0xa3])
        );
        verify_normalized_movie(&files, &project.0.join("build/verify.webm"));
    }
    for (path, original) in paths.iter().zip(originals) {
        assert_eq!(std::fs::read(project.0.join(path)).unwrap(), original);
    }
}

fn verify_normalized_movie(files: &GameFiles, unpacked: &Path) {
    let started = std::time::Instant::now();
    let video = deflorta_assets::video::VideoPlayer::open(
        "intro",
        files,
        "movies/intro.mp4",
        false,
        started,
    );
    assert!(video.size().is_some());
    while video.frame(std::time::Instant::now()).is_none() {
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "converted movie did not decode"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    std::fs::write(unpacked, files.read("movies/intro.mp4").unwrap()).unwrap();
    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_name",
            "-of",
            "json",
        ])
        .arg(unpacked)
        .output()
        .unwrap();
    success(&probe);
    let streams: serde_json::Value = serde_json::from_slice(&probe.stdout).unwrap();
    assert_eq!(streams["streams"][0]["codec_name"], "vp9");
    assert_eq!(streams["streams"][1]["codec_name"], "vorbis");
}

#[test]
fn failed_media_conversion_keeps_the_previous_archive_and_source() {
    let project = Project::new();
    success(&cli(["bundle", project.path()]));
    let archive = project.0.join("build/game.dm");
    let original = std::fs::read(&archive).unwrap();
    project.write("images/broken.png", "invalid image");
    let output = cli(["bundle", project.path()]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("cannot convert media asset 'images/broken.png'")
    );
    assert_eq!(std::fs::read(&archive).unwrap(), original);
    assert_eq!(
        std::fs::read_to_string(project.0.join("images/broken.png")).unwrap(),
        "invalid image"
    );
    assert!(!project.0.join("build/game.dm.partial").exists());
}

#[test]
fn created_game_checks_bundles_and_publishes_without_packing_outputs() {
    let project = Project::new();
    assert!(project.0.join("deflorta.d.ts").is_file());
    success(&cli(["check", project.path()]));
    project.write("releases/stale.txt", "not an asset");
    success(&project.publish("releases"));
    success(&project.publish("releases"));
    let files = GameFiles::open(&project.0.join("releases/game.dm")).unwrap();
    assert!(files.exists("main.js"));
    assert!(files.exists("fonts/NotoSans-Regular.ttf"));
    assert!(!files.exists("deflorta.d.ts"));
    assert!(!files.exists("jsconfig.json"));
    assert_eq!(files.list("releases"), [] as [String; 0]);
    assert_eq!(files.list("build"), [] as [String; 0]);
    assert_eq!(
        std::fs::metadata(project.0.join(format!(
            "releases/test-game{}",
            std::env::consts::EXE_SUFFIX
        )))
        .unwrap()
        .len(),
        std::fs::metadata(distribution().join(format!(
            "target/{}-{}/release/deflorta-launcher{}",
            std::env::consts::OS,
            std::env::consts::ARCH,
            std::env::consts::EXE_SUFFIX
        )))
        .unwrap()
        .len()
    );
    assert_eq!(
        std::fs::read_to_string(project.0.join("releases/resources/runtime.txt")).unwrap(),
        "release"
    );
    let published = project.0.join(format!(
        "releases/test-game{}",
        std::env::consts::EXE_SUFFIX
    ));
    success(
        &Command::new(published)
            .arg("--inspect")
            .current_dir(std::env::temp_dir())
            .output()
            .unwrap(),
    );

    // The custom archive check below is about excluding its own output. Remove
    // the already-verified custom publish folder, which holds a real runtime.
    std::fs::remove_dir_all(project.0.join("releases")).unwrap();
    let archive = project.0.join("custom.dm");
    for _ in 0..2 {
        success(&cli([
            "bundle",
            project.path(),
            "-o",
            archive.to_str().unwrap(),
        ]));
    }
    assert!(!GameFiles::open(&archive).unwrap().exists("custom.dm"));
}

#[test]
fn bundle_preserves_live_imports_scopes_shorthands_defaults_and_cycles() {
    let project = Project::new();
    project.write(
        "state.js",
        r"
export let value = 2;
export function bump() { value += 1; }
export default function () { return value; }
",
    );
    project.write(
        "cycle/a.js",
        r#"
import { b } from "./b.js";
export function a(n) { return n ? b(n - 1) : "a"; }
"#,
    );
    project.write(
        "cycle/b.js",
        r#"
import { a } from "./a.js";
export function b(n) { return n ? a(n - 1) : "b"; }
"#,
    );
    project.write(
        "bridge.js",
        r#"
export * from "./state.js";
export { default } from "./state.js";
export { a } from "./cycle/a.js";
export { configure as setup, label, say } from "deflorta";
"#,
    );
    project.write("asi.js", "export const separate = 1\n");
    project.write("main.js", r#"
import get, { value as imported, bump, a, setup, label, say } from "./bridge.js";
import * as state from "./state.js";
import { separate } from "./asi.js";
(function () { if (separate !== 1) throw new Error("module boundary"); })();
const value = 99;
const Object = 7;
const Symbol = 8;
const shorthand = { imported };
const { imported: copy } = shorthand;
function nested(value) { return imported + value; }
bump();
if (imported !== 3 || state.value !== 3 || get() !== 3 || value !== 99 || copy !== 2 || nested(4) !== 7 || a(2) !== "a" || Object !== 7 || Symbol !== 8) {
  throw new Error("bundle changed module semantics");
}
setup({ id: "module-test", title: "Module Test" });
label("start", async () => { await say("Working!"); });
"#);
    success(&project.publish("dist/test"));
    let files = GameFiles::open(&project.0.join("dist/test/game.dm")).unwrap();
    assert!(!files.exists("state.js"));
    assert!(!files.exists("bridge.js"));
    let code = files.read_to_string("main.js").unwrap();
    assert!(!code.contains("./state.js"));
    assert!(!code.contains("./cycle/"));
    success(&cli(["bundle", project.path(), "--no-minify"]));
}

#[test]
fn jsx_screens_check_translate_bundle_and_boot() {
    let project = Project::new();
    project.write("screens.jsx", r#"
import { View, Text, Pressable, useState, useEffect, _ } from "deflorta";
export default function Panel({ title }) {
  const [count, setCount] = useState(0);
  useEffect(() => { setCount(value => value + 1); }, []);
  return <View key="panel" style={[{ padding: 12 }, { gap: 8 }]}>
    <Text>{_("JSX title")}: {title}: {count}</Text>
    <Pressable onPress={() => setCount(value => value + 1)}><Text>{_("Increment")}</Text></Pressable>
  </View>;
}
"#);
    project.write(
        "main.js",
        r#"
import { configure, screen, showScreen, label, say, Text } from "deflorta";
import Panel from "./screens.jsx";
configure({ id: "jsx-workflow", title: "JSX workflow", font: "Noto Sans" });
screen("panel", () => <><Panel title="works" /><Text key="sibling">Fragment</Text></>, { z: 1000 });
showScreen("panel");
label("start", async () => { await say("Hello JSX"); });
"#,
    );
    success(&cli(["check", project.path()]));
    success(&cli(["translate", "update", "uk", "-p", project.path()]));
    let table: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(project.0.join("tl/uk.json")).unwrap())
            .unwrap();
    assert!(table.get("JSX title").is_some());
    assert!(table.get("Increment").is_some());
    for readable in [true, false] {
        if readable {
            success(&cli(["bundle", project.path(), "--no-minify"]));
        } else {
            success(&cli(["bundle", project.path()]));
        }
        let files = GameFiles::open(&project.0.join("build/game.dm")).unwrap();
        assert!(!files.exists("screens.jsx"));
        let code = files.read_to_string("main.js").unwrap();
        assert!(!code.contains("<View"));
        assert!(code.contains("deflorta/jsx-runtime"));
    }
    success(&project.publish("dist/jsx"));
    let published = project.0.join(format!(
        "dist/jsx/jsx-workflow{}",
        std::env::consts::EXE_SUFFIX
    ));
    success(&Command::new(published).arg("--inspect").output().unwrap());
    let source_boot = Command::new(distribution().join(format!(
        "target/{}-{}/debug/deflorta-launcher{}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        std::env::consts::EXE_SUFFIX
    )))
    .args([project.path(), "--inspect"])
    .output()
    .unwrap();
    assert!(
        !source_boot.status.success(),
        "launchers must not compile source JSX"
    );
    let error = String::from_utf8_lossy(&source_boot.stderr);
    assert!(error.contains("SyntaxError"), "{error}");
    assert!(
        std::fs::read_to_string(project.0.join("main.js"))
            .unwrap()
            .contains("<Panel")
    );
    project.write(
        "screens.jsx",
        r#"
import { Image as Picture, Video, createElement } from "deflorta";
export default () => <>
  <Picture src="missing.png" hoverSrc="missing-hover.png" />
  <Video src="missing.mp4" />
  {createElement(Picture, { src: "missing-classic.png" })}
</>;
"#,
    );
    let output = cli(["check", project.path(), "--no-boot"]);
    assert!(!output.status.success());
    let errors = String::from_utf8_lossy(&output.stdout);
    for file in [
        "missing.png",
        "missing-hover.png",
        "missing.mp4",
        "missing-classic.png",
    ] {
        assert!(errors.contains(file), "{errors}");
    }
    project.write("screens.jsx", "export default () => <View>;");
    assert!(!cli(["check", project.path(), "--no-boot"]).status.success());
    assert!(!cli(["bundle", project.path()]).status.success());
}

#[test]
fn check_and_bundle_reject_broken_imports_and_dynamic_imports() {
    let project = Project::new();
    project.write("main.js", "import { missing } from './missing.js';");
    assert!(!cli(["check", project.path(), "--no-boot"]).status.success());
    assert!(!cli(["bundle", project.path()]).status.success());
    project.write("missing.js", "export const other = 1;");
    let output = cli(["bundle", project.path()]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("no export named 'missing'"));
    project.write("main.js", "const future = import('./missing.js');");
    let output = cli(["bundle", project.path()]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("dynamic import()"));
    project.write("missing.js", "export let value = 1;");
    project.write(
        "main.js",
        "import { value } from './missing.js'; value = 2;",
    );
    let output = cli(["bundle", project.path()]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("read-only"));
}

#[test]
fn local_namespace_facades_check_publish_and_extract_translations() {
    let project = Project::new();
    project.write("api.js", "export * from 'deflorta';");
    project.write(
        "characters.js",
        "import * as api from './api.js'; export const speaker = api.character('Speaker');",
    );
    project.write(
        "main.js",
        r"
import * as api from './api.js';
import * as cast from './characters.js';
api.configure({id: 'namespace-regression', title: 'Namespace regression'});
api.label('start', async () => {
  await api.say('Facade dialogue');
  await cast.speaker('Character dialogue');
  await api.nvlNarrator('NVL dialogue');
});
",
    );
    success(&cli(["check", project.path(), "--no-boot"]));
    success(&cli(["translate", "update", "uk", "-p", project.path()]));
    let table: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(project.0.join("tl/uk.json")).unwrap())
            .unwrap();
    for text in [
        "Speaker",
        "Facade dialogue",
        "Character dialogue",
        "NVL dialogue",
    ] {
        assert!(table.as_object().unwrap().contains_key(text), "{text}");
    }
    success(&project.publish("dist/test"));
}

#[test]
fn bundles_reject_direct_eval_with_source_location() {
    let project = Project::new();
    project.write(
        "other.js",
        "const value = 'other';\nexport function read() { return (eval)('value'); }",
    );
    project.write(
        "main.js",
        r"
import { configure, label } from 'deflorta';
import { read } from './other.js';
configure({id: 'eval-regression'});
const value = 'main';
if (eval('value') !== 'main' || read() !== 'other') throw new Error('wrong scope');
label('start', async () => {});
",
    );
    success(&cli(["check", project.path()]));
    for args in [
        vec!["bundle", project.path()],
        vec!["bundle", project.path(), "--no-minify"],
        vec!["publish", project.path()],
    ] {
        let output =
            Command::new(distribution().join(format!("deflorta{}", std::env::consts::EXE_SUFFIX)))
                .args(args)
                .output()
                .unwrap();
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            error.contains("other.js:2:") && error.contains("direct eval"),
            "{error}"
        );
    }
    assert!(!project.0.join("build/game.dm").exists());
    project.write(
        "other.js",
        "export function read() { return (0, eval)('1 + 1'); }",
    );
    project.write(
        "main.js",
        "import { read } from './other.js'; if (read() !== 2) throw new Error('indirect eval');",
    );
    success(&cli(["bundle", project.path()]));
}

#[cfg(unix)]
#[test]
fn bundling_rejects_outside_file_and_directory_symlinks() {
    use std::os::unix::fs::symlink;
    let outside = Project::new();
    outside.write("sentinel.txt", "outside-root-sentinel");
    let project = Project::new();
    for source in [
        outside.0.join("sentinel.txt"),
        outside.0.clone(),
        project.0.clone(),
    ] {
        let link = project.0.join("linked");
        symlink(source, &link).unwrap();
        let output = cli(["bundle", project.path()]);
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("project symlinks"));
        assert!(!project.0.join("build/game.dm").exists());
        std::fs::remove_file(link).unwrap();
    }
}

#[test]
fn translations_follow_aliases_and_keep_existing_work_until_pruned() {
    let project = Project::new();
    project.write(
        "words.js",
        "export { character as speaker, label, say as line, menu, _ as tr } from 'deflorta';",
    );
    project.write(
        "main.js",
        r#"
import { speaker, label, line, menu, tr } from "./words.js";
const guide = speaker("Guide");
label("start", async () => {
  await guide`Hello!`;
  await line("Narration");
  await menu("Choose", [["Left", 1], { text: "Right", value: 2 }]);
  tr("Explicit");
});
"#,
    );
    project.write(
        "tl/uk.json",
        r#"{"Hello!":"Привіт!","Obsolete":"Old work"}"#,
    );
    success(&cli(["translate", "update", "uk", "-p", project.path()]));
    let table = || -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(project.0.join("tl/uk.json")).unwrap())
            .unwrap()
    };
    assert_eq!(table()["Hello!"], "Привіт!");
    assert_eq!(table()["Obsolete"], "Old work");
    for key in [
        "Guide",
        "Narration",
        "Choose",
        "Left",
        "Right",
        "Explicit",
        "Start",
    ] {
        assert!(
            table().as_object().unwrap().contains_key(key),
            "missing {key}"
        );
    }
    let missing = success(&cli(["translate", "missing", "uk", "-p", project.path()]));
    assert!(missing.contains("\"Narration\""));
    assert!(!missing.contains("\"Hello!\""));
    success(&cli(["translate", "status", "-p", project.path()]));
    success(&cli([
        "translate",
        "update",
        "--prune",
        "-p",
        project.path(),
    ]));
    assert!(table().get("Obsolete").is_none());
    assert_eq!(table()["Hello!"], "Привіт!");
    project.write("tl/uk.json", "{} trailing data");
    assert!(
        !cli(["translate", "update", "uk", "-p", project.path()])
            .status
            .success()
    );
    project.write("tl/uk.json", r#"{"duplicate":null,"duplicate":"text"}"#);
    assert!(
        !cli(["translate", "update", "uk", "-p", project.path()])
            .status
            .success()
    );
}

#[test]
fn create_keeps_placeholder_text_and_types_preserves_editor_settings() {
    let project = Project::new();
    let other = project.0.join("nested");
    success(&cli([
        "create",
        other.to_str().unwrap(),
        "--id",
        "__TITLE__",
        "--title",
        "Quoted \"__ID__\"",
    ]));
    let code = std::fs::read_to_string(other.join("main.js")).unwrap();
    assert!(code.contains("id: \"__TITLE__\""));
    assert!(code.contains(r#"title: "Quoted \"__ID__\"""#));
    success(&cli(["check", other.to_str().unwrap()]));
    project.write("jsconfig.json", "{\"custom\":true}");
    project.write("deflorta.d.ts", "old declarations");
    success(&cli(["types", project.path()]));
    assert_eq!(
        std::fs::read_to_string(project.0.join("jsconfig.json")).unwrap(),
        "{\"custom\":true}"
    );
    assert_eq!(
        std::fs::read_to_string(project.0.join("deflorta.d.ts")).unwrap(),
        std::fs::read_to_string(distribution().join("template/game/deflorta.d.ts")).unwrap()
    );
    assert!(
        !cli([
            "create",
            project.0.join("empty-id").to_str().unwrap(),
            "--id",
            ""
        ])
        .status
        .success()
    );
    let invalid = cli(["publish", project.path(), "--name", "../escaped"]);
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("single valid file name"));
    assert!(!Path::new(project.path()).join("empty-id").exists());
}

#[test]
fn moved_distribution_uses_external_templates_and_reports_missing_data() {
    let project = Project::new();
    let moved = project.0.join(".engine");
    copy_tree(distribution(), &moved);
    let executable = moved.join(format!("deflorta{}", std::env::consts::EXE_SUFFIX));
    let template = moved.join("template/game");
    std::fs::write(template.join(".gitignore"), "custom template\n").unwrap();
    std::fs::write(template.join("images/custom.txt"), "external resource").unwrap();
    std::fs::write(template.join("deflorta.d.ts"), "// external declarations\n").unwrap();
    let created = project.0.join("created");
    success(
        &Command::new(&executable)
            .args(["create", created.to_str().unwrap()])
            .current_dir(std::env::temp_dir())
            .output()
            .unwrap(),
    );
    assert_eq!(
        std::fs::read_to_string(created.join(".gitignore")).unwrap(),
        "custom template\n"
    );
    assert_eq!(
        std::fs::read_to_string(created.join("images/custom.txt")).unwrap(),
        "external resource"
    );
    assert_eq!(
        std::fs::read_to_string(created.join("deflorta.d.ts")).unwrap(),
        "// external declarations\n"
    );
    assert!(created.join("fonts/NotoSans-Regular.ttf").is_file());
    std::fs::remove_dir_all(moved.join("target")).unwrap();
    success(
        &Command::new(&executable)
            .args(["check", created.to_str().unwrap(), "--no-boot"])
            .output()
            .unwrap(),
    );
    let output = Command::new(&executable)
        .args(["check", created.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("launcher"));
    std::fs::remove_dir_all(moved.join("template")).unwrap();
    let missing = project.0.join("missing-template");
    let output = Command::new(&executable)
        .args(["create", missing.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("template folder"));
    assert!(!missing.exists());
}

#[test]
fn publish_selects_profiles_and_foreign_platform_resources() {
    let project = Project::new();
    success(&cli([
        "publish",
        project.path(),
        "--debug",
        "--level",
        "12",
    ]));
    for level in ["0", "13"] {
        assert!(
            !cli(["bundle", project.path(), "--level", level])
                .status
                .success()
        );
        assert!(
            !cli(["publish", project.path(), "--level", level])
                .status
                .success()
        );
    }
    let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    assert_eq!(
        std::fs::read_to_string(
            project
                .0
                .join("dist")
                .join(platform)
                .join("resources/runtime.txt")
        )
        .unwrap(),
        "debug"
    );
    let conflict = cli(["publish", project.path(), "--name", "resources"]);
    assert!(!conflict.status.success());
    assert!(
        String::from_utf8_lossy(&conflict.stderr).contains("conflicts with a runtime resource")
    );
    let moved = project.0.join(".engine");
    copy_tree(distribution(), &moved);
    let foreign = moved.join("target/windows-x86_64/release");
    std::fs::create_dir_all(foreign.join("resources")).unwrap();
    std::fs::write(
        foreign.join("deflorta-launcher.exe"),
        "foreign binary; must never execute here",
    )
    .unwrap();
    std::fs::write(foreign.join("resources/engine.dat"), "foreign resource").unwrap();
    let output = project.0.join("dist/windows");
    success(
        &Command::new(moved.join(format!("deflorta{}", std::env::consts::EXE_SUFFIX)))
            .args([
                "publish",
                project.path(),
                "--platform",
                "windows-x86_64",
                "-o",
                output.to_str().unwrap(),
            ])
            .output()
            .unwrap(),
    );
    assert_eq!(
        std::fs::read_to_string(output.join("test-game.exe")).unwrap(),
        "foreign binary; must never execute here"
    );
    assert_eq!(
        std::fs::read_to_string(output.join("resources/engine.dat")).unwrap(),
        "foreign resource"
    );
    assert!(!output.join("deflorta-launcher.exe").exists());
}

#[cfg(unix)]
#[test]
fn android_publish_packages_verified_game_and_selected_native_profile() {
    let project = Project::new();
    project.write(
        "main.js",
        r#"
import { configure, label, say } from "deflorta";
configure({ id: "test-game", title: "Android export", font: "Noto Sans", version: "2.4" });
label("start", async () => { await say("Hello Android"); });
"#,
    );
    let moved = project.0.join(".engine");
    copy_tree(distribution(), &moved);
    for mode in ["debug", "release"] {
        let libraries = moved.join(format!("target/android-aarch64/{mode}/jniLibs/arm64-v8a"));
        std::fs::create_dir_all(&libraries).unwrap();
        std::fs::write(libraries.join("libdeflorta.so"), mode).unwrap();
        std::fs::write(libraries.join("libc++_shared.so"), "C++ runtime").unwrap();
    }
    // The real host launcher verifies startup; substitute only the Gradle
    // build so this packaging regression needs no SDK or signing credentials.
    std::fs::write(
        moved.join("template/android/gradlew"),
        r#"
set -eu
test "$1" = "--console=plain"
test "$2" = "assembleDebug"
mkdir -p app/build/outputs/apk/debug
cp app/src/main/assets/game.dm app/build/outputs/apk/debug/app-debug.apk
"#,
    )
    .unwrap();
    let publish = |package: &str| {
        Command::new(moved.join("deflorta"))
            .args([
                "publish",
                project.path(),
                "--platform",
                "android-aarch64",
                "--debug",
                "--android-package",
                package,
                "--android-version-code",
                "7",
            ])
            .output()
            .unwrap()
    };
    let invalid = publish("com.invalid-package");
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("Android package"));
    assert!(!project.0.join("dist/android-aarch64/android").exists());
    success(&publish("com.example.mygame"));
    let output = project.0.join("dist/android-aarch64");
    let gradle = output.join("android");
    let libraries = gradle.join("app/src/main/jniLibs/arm64-v8a");
    assert_eq!(
        std::fs::read(libraries.join("libdeflorta.so")).unwrap(),
        b"debug"
    );
    assert!(libraries.join("libc++_shared.so").is_file());
    let config: serde_json::Value =
        serde_json::from_slice(&std::fs::read(gradle.join("deflorta-export.json")).unwrap())
            .unwrap();
    assert_eq!(config["applicationId"], "com.example.mygame");
    assert_eq!(config["versionCode"], 7);
    assert_eq!(config["versionName"], "2.4");
    let artifact = GameFiles::open(&output.join("test-game.apk")).unwrap();
    assert!(artifact.exists("main.js"));
    assert!(!artifact.exists("game.dm"));
    std::fs::create_dir_all(gradle.join("app/src/main/jniLibs/x86_64")).unwrap();
    std::fs::write(gradle.join("app/src/main/jniLibs/x86_64/stale.so"), "stale").unwrap();
    success(&publish("com.example.mygame"));
    assert!(!gradle.join("app/src/main/jniLibs/x86_64").exists());
    let unsigned = Command::new(moved.join("deflorta"))
        .args(["publish", project.path(), "--platform", "android-aarch64"])
        .env_remove("DEFLORTA_KEYSTORE")
        .output()
        .unwrap();
    assert!(!unsigned.status.success());
    assert!(
        String::from_utf8_lossy(&unsigned.stderr)
            .contains("release Android exports need DEFLORTA_KEYSTORE")
    );
    std::fs::rename(&gradle, output.join("previous-android")).unwrap();
    std::fs::create_dir(&gradle).unwrap();
    std::fs::write(gradle.join("custom.txt"), "keep me").unwrap();
    let custom = publish("com.example.mygame");
    assert!(!custom.status.success());
    assert!(String::from_utf8_lossy(&custom.stderr).contains("not a generated Android export"));
    assert_eq!(
        std::fs::read_to_string(gradle.join("custom.txt")).unwrap(),
        "keep me"
    );
    let desktop = cli([
        "publish",
        project.path(),
        "--android-package",
        "com.example.game",
    ]);
    assert!(!desktop.status.success());
    assert!(String::from_utf8_lossy(&desktop.stderr).contains("require --platform"));
}

#[cfg(unix)]
#[test]
fn run_forwards_arguments_environment_and_exit_status_to_debug_launcher() {
    let project = Project::new();
    project.write(
        "main.js",
        "import { Text } from 'deflorta'; export const view = <Text>Compiled</Text>;",
    );
    let moved = project.0.join(".engine");
    copy_tree(distribution(), &moved);
    let launcher = moved.join(format!(
        "target/{}-{}/debug/deflorta-launcher",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    std::fs::write(
        launcher,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" \"$RUST_LOG\"\ntest \"$2\" = --scripts || exit 9\ncase \"$(cat \"$3/main.js\")\" in *'<Text'*) exit 9 ;; *'deflorta/jsx-runtime'*) printf 'compiled\\n' ;; *) exit 9 ;; esac\nexit 7\n",
    )
    .unwrap();
    let output = Command::new(moved.join("deflorta"))
        .args(["run", project.path(), "--test", "steps.json", "-vv"])
        .env("RUST_LOG", "deflorta=trace")
        .current_dir(std::env::temp_dir())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(7));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let args: Vec<_> = stdout.lines().collect();
    assert_eq!(args.len(), 8, "{stdout}");
    assert_eq!(args[0], project.path());
    assert_eq!(args[1], "--scripts");
    assert!(
        !Path::new(args[2]).exists(),
        "compiled scripts must be cleaned up"
    );
    assert_eq!(args[3], "--test");
    assert_eq!(Path::new(args[4]), std::env::temp_dir().join("steps.json"));
    assert_eq!(args[5..], ["-vv", "deflorta=trace", "compiled"]);
}
