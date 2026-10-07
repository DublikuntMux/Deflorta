//! Exercise the real CLI, including `SpiderMonkey` evaluation of published bundles.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use deflorta_data::GameFiles;

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
            .args(["build", "--locked", "-p", "deflorta-launcher"]);
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
            root.join("crates/engine/runtime/deflorta.d.ts"),
            template.join("deflorta.d.ts"),
        )
        .unwrap();
        for dir in ["images", "audio", "movies", "tl"] {
            std::fs::create_dir_all(template.join(dir)).unwrap();
        }
        copy_tree(
            &root.join("crates/engine/runtime"),
            &path.join("template/runtime"),
        );
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
fn run_forwards_arguments_environment_and_exit_status_to_debug_launcher() {
    let project = Project::new();
    let moved = project.0.join(".engine");
    copy_tree(distribution(), &moved);
    let launcher = moved.join(format!(
        "target/{}-{}/debug/deflorta-launcher",
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    std::fs::write(
        launcher,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" \"$RUST_LOG\"\nexit 7\n",
    )
    .unwrap();
    let output = Command::new(moved.join("deflorta"))
        .args(["run", project.path(), "--test", "steps.json", "-vv"])
        .env("RUST_LOG", "deflorta=trace")
        .current_dir(std::env::temp_dir())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!(
            "{}\n--test\n{}\n-vv\ndeflorta=trace\n",
            project.path(),
            std::env::temp_dir().join("steps.json").display()
        )
    );
}
