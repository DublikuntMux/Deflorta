//! Native modules (TurboModule-style): typed functions installed on
//! `__native` and called synchronously from JavaScript. Arguments are read
//! straight from JS values; calls that affect the engine queue a typed
//! [`Command`] that the engine applies as soon as the JS call returns.

use std::collections::HashMap;
use std::ffi::CStr;
use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use log::Level;
use mozjs::context::RawJSContext;
use mozjs::gc::{Handle, MutableHandle, RootedTraceableBox};
use mozjs::jsapi::{self, CallArgs, Heap, JSObject, Value};
use mozjs::jsval::ObjectValue;
use mozjs::rooted;
use num_traits::ToPrimitive;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::value::{self, Handler, HandlerSink, from_js, from_js_with_handlers, to_js};
use super::{HandlerTable, throw_error, with_state};
use crate::assets::normalize_game_path;
use crate::ui::desc::{AnimDesc, Color, NodeDesc};

// ---------------------------------------------------------------------------
// Spec: the types exchanged with JavaScript
// ---------------------------------------------------------------------------

/// Events delivered to the runtime's dispatch function.
#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Event<'a> {
    Boot,
    Quit,
    /// All typewriter text finished revealing.
    Revealed,
    Click {
        /// The clicked element's `onClick`, or null for the background.
        handler: Option<Handler>,
        button: &'a str,
        revealing: bool,
    },
    /// A widget handler (`onChange`, `onInput`, `onSubmit`, `onEnd`).
    Handler {
        handler: Handler,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<HandlerValue>,
    },
    Key {
        key: &'a str,
        down: bool,
        repeat: bool,
        ctrl: bool,
        shift: bool,
        alt: bool,
        revealing: bool,
    },
    Wheel {
        dy: f32,
        revealing: bool,
    },
    Tooltip {
        text: Option<String>,
    },
    Timer {
        id: u64,
    },
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum HandlerValue {
    Number(f32),
    Text(String),
}

/// Engine work requested by native module calls, in call order.
pub enum Command {
    Configure(GameConfig),
    SetTimer {
        id: u64,
        ms: f64,
    },
    ClearTimer {
        id: u64,
    },
    /// Changes the music; `None` stops it.
    Music(Option<Music>, Fade),
    Sound {
        file: String,
        volume: f32,
    },
    /// Plays a voice line, stopping the previous one; `None` stops voice.
    Voice {
        file: Option<String>,
    },
    Volume {
        channel: String,
        value: f32,
    },
    /// Shows all text up to the next click-wait (or the end).
    RevealSkip,
    /// Starts decoding images in the background.
    Preload {
        images: Vec<String>,
    },
    /// Captures a frame for a save thumbnail: the screen as it is now (before
    /// the next tree is shown, e.g. under a menu that is opening), or with
    /// `after`, once the next tree is shown (e.g. a scene that is starting).
    CaptureThumbnail {
        after: bool,
    },
    /// Writes the last captured thumbnail to `<data dir>/<name>.png`.
    SaveThumbnail {
        name: String,
    },
    /// Removes `<data dir>/<name>.png` (when its save is deleted).
    DeleteThumbnail {
        name: String,
    },
    Fullscreen {
        on: bool,
    },
    Quit,
    /// A new element tree from `ui.commit`.
    Commit(Box<UiCommit>),
}

pub struct UiCommit {
    pub tree: NodeDesc,
    /// Generation of the handlers captured from the tree.
    pub generation: u32,
    pub instant: bool,
    /// Exit animations for keyed elements removed in this commit.
    pub exits: HashMap<String, Option<AnimDesc>>,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GameConfig {
    pub id: String,
    pub title: String,
    pub width: f32,
    pub height: f32,
    pub font: String,
    /// Game version as set by the script (any JSON value, e.g. "1.2" or 3).
    #[serde(default)]
    pub version: Option<serde_json::Value>,
    #[serde(default)]
    pub clear_color: Option<Color>,
}

#[derive(Deserialize)]
pub struct Music {
    pub file: String,
    #[serde(default = "default_true")]
    pub r#loop: bool,
    #[serde(default = "default_one")]
    pub volume: f32,
}

#[derive(Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct Fade {
    #[serde(default)]
    pub fade_in: f32,
    #[serde(default)]
    pub fade_out: f32,
}

const fn default_true() -> bool {
    true
}

const fn default_one() -> f32 {
    1.0
}

#[derive(Deserialize, Default)]
struct CommitOptions {
    #[serde(default)]
    instant: bool,
    #[serde(default)]
    exits: HashMap<String, Option<AnimDesc>>,
}

#[derive(Serialize)]
struct DataEntry {
    name: String,
    /// Milliseconds since the Unix epoch.
    modified: u64,
}

// ---------------------------------------------------------------------------
// Installation
// ---------------------------------------------------------------------------

type Native = unsafe extern "C" fn(*mut RawJSContext, u32, *mut Value) -> bool;
type Module = &'static [(&'static CStr, Native, u32)];

const ROOT: Module = &[(c"log", log_message, 2), (c"connect", connect, 2)];

const MODULES: &[(&CStr, Module)] = &[
    (c"files", &[(c"readText", files_read_text, 1)]),
    (
        c"storage",
        &[
            (c"read", storage_read, 1),
            (c"write", storage_write, 2),
            (c"remove", storage_remove, 1),
            (c"list", storage_list, 0),
        ],
    ),
    (
        c"timers",
        &[(c"set", timers_set, 2), (c"clear", timers_clear, 1)],
    ),
    (
        c"app",
        &[
            (c"configure", app_configure, 1),
            (c"fullscreen", app_fullscreen, 1),
            (c"quit", app_quit, 0),
        ],
    ),
    (
        c"audio",
        &[
            (c"music", audio_music, 2),
            (c"sound", audio_sound, 2),
            (c"voice", audio_voice, 1),
            (c"volume", audio_volume, 2),
        ],
    ),
    (
        c"ui",
        &[
            (c"commit", ui_commit, 2),
            (c"revealSkip", ui_reveal_skip, 0),
            (c"preload", ui_preload, 1),
            (c"captureThumbnail", ui_capture_thumbnail, 1),
            (c"saveThumbnail", ui_save_thumbnail, 1),
            (c"deleteThumbnail", ui_delete_thumbnail, 1),
        ],
    ),
];

/// Defines `globalThis.__native` with the root functions and one object per module.
pub unsafe fn install(cx: *mut RawJSContext, global: *mut JSObject) -> Result<()> {
    unsafe {
        rooted!(in(cx) let global = global);
        rooted!(in(cx) let native = jsapi::JS_NewPlainObject(cx));
        if native.get().is_null() {
            bail!("failed to create __native");
        }
        define_functions(cx, native.handle(), ROOT)?;
        for (name, functions) in MODULES {
            rooted!(in(cx) let module = jsapi::JS_NewPlainObject(cx));
            if module.get().is_null() {
                bail!("failed to create __native.{}", name.to_string_lossy());
            }
            define_functions(cx, module.handle(), functions)?;
            define_object(cx, native.handle(), name, module.get())?;
        }
        define_object(cx, global.handle(), c"__native", native.get())
    }
}

unsafe fn define_functions(
    cx: *mut RawJSContext,
    obj: Handle<*mut JSObject>,
    functions: Module,
) -> Result<()> {
    for (name, native, nargs) in functions {
        let f = unsafe {
            jsapi::JS_DefineFunction(cx, obj.into(), name.as_ptr(), Some(*native), *nargs, 0)
        };
        if f.is_null() {
            bail!(
                "failed to define native function {}",
                name.to_string_lossy()
            );
        }
    }
    Ok(())
}

unsafe fn define_object(
    cx: *mut RawJSContext,
    target: Handle<*mut JSObject>,
    name: &CStr,
    obj: *mut JSObject,
) -> Result<()> {
    unsafe {
        rooted!(in(cx) let value = ObjectValue(obj));
        if !jsapi::JS_DefineProperty(cx, target.into(), name.as_ptr(), value.handle().into(), 0) {
            bail!("failed to define {}", name.to_string_lossy());
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Calling convention
// ---------------------------------------------------------------------------

struct Args {
    cx: *mut RawJSContext,
    call: CallArgs,
}

impl Args {
    /// Reads argument `index` (missing arguments are `undefined`).
    fn get<T: DeserializeOwned>(&self, index: u32) -> Result<T, value::Error> {
        unsafe { from_js(self.cx, Handle::from_raw(self.call.get(index))) }
    }

    fn handle(&self, index: u32) -> Handle<'_, Value> {
        unsafe { Handle::from_raw(self.call.get(index)) }
    }
}

/// Runs a native body, converts its result to JS and turns errors into exceptions.
unsafe fn call<R: Serialize>(
    cx: *mut RawJSContext,
    argument_count: u32,
    vp: *mut Value,
    body: impl FnOnce(&Args) -> Result<R>,
) -> bool {
    let args = Args {
        cx,
        call: unsafe { CallArgs::from_vp(vp, argument_count) },
    };
    let result = body(&args).and_then(|value| {
        let rval = unsafe { MutableHandle::from_raw(args.call.rval()) };
        unsafe { to_js(cx, &value, rval)? };
        Ok(())
    });
    match result {
        Ok(()) => true,
        Err(err) => {
            // Keep an exception thrown by JS code we called (e.g. a getter).
            if !unsafe { jsapi::JS_IsExceptionPending(cx) } {
                unsafe { throw_error(cx, &format!("{err:#}")) };
            }
            false
        }
    }
}

macro_rules! native {
    ($($(#[$attr:meta])* fn $name:ident($args:ident) $body:block)*) => {$(
        $(#[$attr])*
        unsafe extern "C" fn $name(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
            let body = |$args: &Args| $body;
            unsafe { call(cx, argc, vp, body) }
        }
    )*};
}

fn queue(command: Command) {
    with_state(|s| s.commands.push(command));
}

// ---------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------

native! {
    fn log_message(args) {
        let level: String = args.get(0)?;
        let message: String = args.get(1)?;
        let level = match level.as_str() {
            "error" => Level::Error,
            "warn" => Level::Warn,
            "debug" => Level::Debug,
            "trace" => Level::Trace,
            _ => Level::Info,
        };
        log::log!(target: "deflorta::js", level, "{message}");
        Ok(())
    }

    /// `connect(dispatch, flush)`: the runtime's entry points for events and
    /// for committing pending output at the end of each turn.
    fn connect(args) {
        let functions = [args.handle(0), args.handle(1)].map(|f| {
            let callable = f.get().is_object() && unsafe { jsapi::IsCallable(f.get().to_object()) };
            callable.then(|| RootedTraceableBox::from_box(Heap::boxed(f.get())))
        });
        let [Some(dispatch), Some(flush)] = functions else {
            bail!("connect(dispatch, flush) expects two functions");
        };
        with_state(|s| s.entry_points = Some([dispatch, flush]));
        Ok(())
    }

    fn files_read_text(args) {
        let path: String = args.get(0)?;
        Ok(normalize_game_path(Path::new(&path))
            .and_then(|p| std::fs::read_to_string(with_state(|s| s.game_dir.join(p))).ok()))
    }

    fn storage_read(args) {
        let name: String = args.get(0)?;
        Ok(data_path(&name).and_then(|path| std::fs::read_to_string(path).ok()))
    }

    fn storage_write(args) {
        let name: String = args.get(0)?;
        let text: String = args.get(1)?;
        let Some(path) = data_path(&name) else {
            bail!("storage.write: invalid name '{name}' or data directory not configured");
        };
        // Write to a temporary file first so a crash never leaves a torn save.
        let tmp = path.with_extension("json.tmp");
        path.parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&tmp, text))
            .and_then(|()| std::fs::rename(&tmp, &path))
            .map_err(|err| anyhow::anyhow!("storage.write failed: {err}"))
    }

    fn storage_remove(args) {
        let name: String = args.get(0)?;
        Ok(data_path(&name).is_some_and(|path| std::fs::remove_file(path).is_ok()))
    }

    fn storage_list(_args) {
        Ok(list_data())
    }

    fn timers_set(args) {
        let id: u64 = args.get(0)?;
        let ms: f64 = args.get(1)?;
        queue(Command::SetTimer { id, ms: ms.max(0.0) });
        Ok(())
    }

    fn timers_clear(args) {
        queue(Command::ClearTimer { id: args.get(0)? });
        Ok(())
    }

    fn app_configure(args) {
        queue(Command::Configure(args.get(0)?));
        Ok(())
    }

    fn app_fullscreen(args) {
        queue(Command::Fullscreen { on: args.get(0)? });
        Ok(())
    }

    fn app_quit(_args) {
        queue(Command::Quit);
        Ok(())
    }

    /// `music({ file, loop, volume } | null, { fadeIn, fadeOut })`
    fn audio_music(args) {
        let fade: Option<Fade> = args.get(1)?;
        queue(Command::Music(args.get(0)?, fade.unwrap_or_default()));
        Ok(())
    }

    fn audio_sound(args) {
        let file: String = args.get(0)?;
        let volume: Option<f32> = args.get(1)?;
        queue(Command::Sound { file, volume: volume.unwrap_or(1.0) });
        Ok(())
    }

    fn audio_voice(args) {
        queue(Command::Voice { file: args.get(0)? });
        Ok(())
    }

    fn audio_volume(args) {
        queue(Command::Volume { channel: args.get(0)?, value: args.get(1)? });
        Ok(())
    }

    /// `commit(tree, { instant, exits })`: replaces the element tree. The tree
    /// is read in place; its functions stay in JS and are referenced by handle.
    fn ui_commit(args) {
        unsafe { commit(args) }
    }

    fn ui_reveal_skip(_args) {
        queue(Command::RevealSkip);
        Ok(())
    }

    fn ui_preload(args) {
        queue(Command::Preload { images: args.get(0)? });
        Ok(())
    }

    fn ui_capture_thumbnail(args) {
        let after: Option<bool> = args.get(0)?;
        queue(Command::CaptureThumbnail { after: after.unwrap_or(false) });
        Ok(())
    }

    fn ui_save_thumbnail(args) {
        queue(Command::SaveThumbnail { name: args.get(0)? });
        Ok(())
    }

    fn ui_delete_thumbnail(args) {
        queue(Command::DeleteThumbnail { name: args.get(0)? });
        Ok(())
    }
}

unsafe fn commit(args: &Args) -> Result<()> {
    let cx = args.cx;
    let generation = with_state(super::HostState::next_generation);
    unsafe {
        rooted!(in(cx) let handlers = jsapi::NewArrayObject1(cx, 0));
        if handlers.get().is_null() {
            bail!("cannot create the handler table");
        }
        let sink = HandlerSink::new(generation, handlers.handle());
        let tree: NodeDesc = from_js_with_handlers(cx, args.handle(0), &sink)?;
        let options: Option<CommitOptions> = args.get(1)?;
        let options = options.unwrap_or_default();
        let table = HandlerTable {
            generation,
            functions: RootedTraceableBox::from_box(Heap::boxed(handlers.get())),
        };
        with_state(|s| {
            s.handlers.push_back(table);
            s.commands.push(Command::Commit(Box::new(UiCommit {
                tree,
                generation,
                instant: options.instant,
                exits: options.exits,
            })));
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Storage helpers
// ---------------------------------------------------------------------------

fn is_valid_data_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
}

fn data_path(name: &str) -> Option<PathBuf> {
    if !is_valid_data_name(name) {
        return None;
    }
    with_state(|s| s.data_dir.as_ref().map(|d| d.join(format!("{name}.json"))))
}

fn list_data() -> Vec<DataEntry> {
    let mut entries = Vec::new();
    let Some(dir) = with_state(|s| s.data_dir.clone()) else {
        return entries;
    };
    let Ok(read) = std::fs::read_dir(dir) else {
        return entries;
    };
    for entry in read.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let modified = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_millis().to_u64().unwrap_or(u64::MAX));
        entries.push(DataEntry {
            name: name.to_owned(),
            modified,
        });
    }
    entries
}
