//! Exercise the real CLI, including `SpiderMonkey` evaluation of published bundles.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

use deflorta::GameFiles;

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
        // Verify bundled startup and copied launcher bytes without requiring a
        // window server or audio hardware in the test runner.
        self.write("build/launcher", "launcher fixture");
        cli([
            "publish",
            self.path(),
            "--launcher",
            self.0.join("build/launcher").to_str().unwrap(),
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
    Command::new(env!("CARGO_BIN_EXE_deflorta"))
        .args(args)
        // Avoid developer preferences/saves affecting a fresh project's boot.
        .env(
            "XDG_DATA_HOME",
            std::env::temp_dir().join("deflorta-cli-test-data"),
        )
        .output()
        .unwrap()
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
        std::fs::read_to_string(project.0.join(format!(
            "releases/test-game{}",
            std::env::consts::EXE_SUFFIX
        )))
        .unwrap(),
        "launcher fixture"
    );

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
        deflorta::TYPE_DECLARATIONS
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
