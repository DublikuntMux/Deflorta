//! `SpiderMonkey` host: ES module loading, the microtask queue, and the
//! synchronous bridge to the JavaScript runtime. Events are passed to JS as
//! objects, and JS calls typed native modules (`native.rs`) whose values are
//! read in place (`value.rs`); no JSON crosses the boundary.

mod native;
#[cfg(test)]
mod tests;
mod value;

pub use native::{Command, Event, GameConfig, HandlerValue, UiCommit};
pub use value::Handler;

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::ffi::CString;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::ptr::{self, NonNull};
use std::time::Instant;

use anyhow::{Result, anyhow, bail};
use log::{debug, error, info};
use mozjs::context::{JSContext, RawJSContext};
use mozjs::conversions::{
    ConversionResult, FromJSValConvertible, ToJSValConvertible, jsstr_to_string,
};
use mozjs::gc::{Handle, MutableHandle, RootedTraceableBox};
use mozjs::jsapi::{
    self, ColumnNumberOneOrigin, Heap, JSObject, JSScript, OnNewGlobalHookOption, PromiseState,
    Value,
};
use mozjs::jsval::{NullValue, UndefinedValue};
use mozjs::rooted;
use mozjs::rust::{
    CompileOptionsWrapper, JSEngine, RealmOptions, Runtime, SIMPLE_GLOBAL_CLASS,
    transform_str_to_source_text,
};

use crate::files::{GameFiles, normalize_game_path};

/// Embeds a runtime module prepared by the build script (minified in release).
macro_rules! runtime_module {
    ($name:literal) => {
        include_str!(concat!(env!("OUT_DIR"), "/runtime/", $name, ".js"))
    };
}

/// Built-in modules, importable by bare specifier: `(specifier, source)`.
/// Sources are minified in release builds.
pub const BUILTIN_MODULES: &[(&str, &str)] = &[
    ("deflorta", runtime_module!("deflorta")),
    ("deflorta/core", runtime_module!("core")),
    ("deflorta/ui", runtime_module!("ui")),
    ("deflorta/text", runtime_module!("text")),
    ("deflorta/scene", runtime_module!("scene")),
    ("deflorta/story", runtime_module!("story")),
    ("deflorta/screens", runtime_module!("screens")),
];

const BOOT_MODULE: &str = "import \"deflorta\";\nimport \"./main.js\";\n";

/// Per-thread state shared with native callbacks and the module loader hook.
struct HostState {
    files: GameFiles,
    data_dir: Option<PathBuf>,
    modules: HashMap<String, RootedTraceableBox<Heap<*mut JSObject>>>,
    load_error: Option<String>,
    /// The runtime's `dispatch(event)` and `flush()` functions.
    entry_points: Option<[RootedTraceableBox<Heap<Value>>; 2]>,
    /// Commands queued by native calls since the engine last took them.
    commands: Vec<Command>,
    /// Functions of committed trees, oldest first, until the engine releases them.
    handlers: VecDeque<HandlerTable>,
    generation: u32,
}

/// The functions captured from one `ui.commit`, as a JS array.
struct HandlerTable {
    generation: u32,
    functions: RootedTraceableBox<Heap<*mut JSObject>>,
}

impl HostState {
    const fn next_generation(&mut self) -> u32 {
        self.generation = (self.generation + 1) % Handler::GENERATIONS;
        self.generation
    }
}

thread_local! {
    static STATE: RefCell<Option<HostState>> = const { RefCell::new(None) };
}

fn with_state<R>(f: impl FnOnce(&mut HostState) -> R) -> R {
    STATE.with(|s| {
        f(s.borrow_mut()
            .as_mut()
            .expect("script host not initialized"))
    })
}

pub struct ScriptHost {
    global: RootedTraceableBox<Heap<*mut JSObject>>,
    runtime: Runtime,
    _engine: JSEngine,
}

impl ScriptHost {
    pub fn new(files: GameFiles) -> Result<Self> {
        debug!("Initializing SpiderMonkey");
        let engine = JSEngine::init().map_err(|e| anyhow!("failed to init SpiderMonkey: {e:?}"))?;
        let mut runtime = Runtime::new(engine.handle());
        let cx = unsafe { runtime.cx().raw_cx() };

        STATE.with(|s| {
            *s.borrow_mut() = Some(HostState {
                files,
                data_dir: None,
                modules: HashMap::new(),
                load_error: None,
                entry_points: None,
                commands: Vec::new(),
                handlers: VecDeque::new(),
                generation: 0,
            });
        });

        unsafe {
            // SpiderMonkey owns the microtask queue; we drain it in `run_jobs`.
            // The queue lives for the whole process, so it is intentionally leaked.
            let queue = mozjs::glue::CreateJobQueue(&raw const JOB_QUEUE_TRAPS);
            jsapi::SetJobQueue(cx, queue.cast::<jsapi::JobQueue>());
            jsapi::SetModuleLoadHook(jsapi::JS_GetRuntime(cx), Some(module_load_hook));

            let options = RealmOptions::default();
            let global = jsapi::JS_NewGlobalObject(
                cx,
                &raw const SIMPLE_GLOBAL_CLASS,
                ptr::null_mut(),
                OnNewGlobalHookOption::FireOnNewGlobalHook,
                &raw const *options,
            );
            if global.is_null() {
                bail!("failed to create the global object");
            }
            let global = RootedTraceableBox::from_box(Heap::boxed(global));
            // The engine lives in a single realm for the whole program.
            jsapi::EnterRealm(cx, global.get());
            if !jsapi::InitRealmStandardClasses(cx) {
                bail!("failed to init standard classes");
            }
            native::install(cx, global.get())?;
            info!(
                "Script runtime ready (SpiderMonkey, {} built-in modules)",
                BUILTIN_MODULES.len()
            );

            Ok(Self {
                global,
                runtime,
                _engine: engine,
            })
        }
    }

    fn cx(&mut self) -> *mut RawJSContext {
        unsafe { self.runtime.cx().raw_cx() }
    }

    /// Loads the runtime and `main.js`, evaluates the module graph and drains jobs.
    pub fn run_main(&mut self) -> Result<()> {
        let started = Instant::now();
        let cx = self.cx();
        unsafe {
            let module = compile_module(cx, "__boot__.js", BOOT_MODULE)
                .map_err(|_| pending_exception(cx))?;
            rooted!(in(cx) let module = module);
            rooted!(in(cx) let host_defined = UndefinedValue());
            if !jsapi::LoadRequestedModules(
                cx,
                module.handle().into(),
                host_defined.handle().into(),
                Some(on_modules_loaded),
                Some(on_modules_failed),
            ) {
                return Err(pending_exception(cx));
            }
            run_jobs(cx);
            if let Some(err) = with_state(|s| s.load_error.take()) {
                bail!("{err}");
            }
            if !jsapi::ModuleLink(cx, module.handle().into()) {
                return Err(pending_exception(cx));
            }
            rooted!(in(cx) let mut rval = UndefinedValue());
            if !jsapi::ModuleEvaluate(cx, module.handle().into(), rval.handle_mut().into()) {
                return Err(pending_exception(cx));
            }
            run_jobs(cx);
            if rval.get().is_object() {
                rooted!(in(cx) let promise = rval.get().to_object());
                if jsapi::GetPromiseState(promise.handle().into()) == PromiseState::Rejected {
                    rooted!(in(cx) let mut reason = UndefinedValue());
                    mozjs::glue::JS_GetPromiseResult(
                        promise.handle().into(),
                        reason.handle_mut().into(),
                    );
                    bail!("{}", describe_error(cx, reason.handle()));
                }
            }
        }
        let count = with_state(|s| s.modules.len());
        info!("Loaded {count} script modules in {:.0?}", started.elapsed());
        Ok(())
    }

    /// Delivers an event to the JS runtime, runs promise jobs so story code
    /// can advance to its next suspension point, then lets the runtime commit
    /// its output. The resulting commands are available from `take_commands`.
    pub fn dispatch(&mut self, event: &Event) -> Result<()> {
        let cx = self.cx();
        let result = unsafe {
            rooted!(in(cx) let mut arg = UndefinedValue());
            value::to_js(cx, event, arg.handle_mut())?;
            self.call_entry_point(0, Some(arg.handle()))
        };
        unsafe { run_jobs(cx) };
        let flushed = self.flush();
        result.and(flushed)
    }

    /// Lets the runtime commit pending output (UI tree, music) through native calls.
    pub fn flush(&mut self) -> Result<()> {
        let result = unsafe { self.call_entry_point(1, None) };
        let cx = self.cx();
        unsafe { run_jobs(cx) };
        result
    }

    /// Commands queued by native module calls, in call order.
    pub fn take_commands() -> Vec<Command> {
        with_state(|s| std::mem::take(&mut s.commands))
    }

    /// Drops the functions of trees committed before `generation`; their
    /// handlers can no longer be triggered.
    pub fn release_handlers(generation: u32) {
        with_state(|s| {
            if !s.handlers.iter().any(|t| t.generation == generation) {
                return;
            }
            while s
                .handlers
                .front()
                .is_some_and(|t| t.generation != generation)
            {
                s.handlers.pop_front();
            }
        });
    }

    pub fn maybe_gc(&mut self) {
        let cx = self.cx();
        unsafe { jsapi::JS_MaybeGC(cx) };
    }

    pub fn set_data_dir(dir: PathBuf) {
        info!("User data directory: {}", dir.display());
        with_state(|s| s.data_dir = Some(dir));
    }

    /// Calls `dispatch` (0) or `flush` (1) as registered by `native.connect`.
    unsafe fn call_entry_point(&mut self, which: usize, arg: Option<Handle<Value>>) -> Result<()> {
        let global = self.global.get();
        let cx = self.cx();
        unsafe {
            rooted!(in(cx) let function = with_state(|s| {
                s.entry_points.as_ref().map_or_else(UndefinedValue, |functions| functions[which].get())
            }));
            if function.get().is_undefined() {
                bail!("the runtime did not connect to the engine");
            }
            rooted!(in(cx) let global = global);
            rooted!(in(cx) let mut rval = UndefinedValue());
            let args = arg.map_or_else(jsapi::HandleValueArray::empty, |arg| {
                let raw: jsapi::HandleValue = arg.into();
                jsapi::HandleValueArray::from(raw)
            });
            if !jsapi::JS_CallFunctionValue(
                cx,
                global.handle().into(),
                function.handle().into(),
                &raw const args,
                rval.handle_mut().into(),
            ) {
                return Err(pending_exception(cx));
            }
        }
        Ok(())
    }
}

/// Writes the function behind `handler` to `out`, or null once released.
unsafe fn handler_function(cx: *mut RawJSContext, handler: Handler, mut out: MutableHandle<Value>) {
    out.set(NullValue());
    with_state(|s| {
        let Some(table) = s
            .handlers
            .iter()
            .find(|t| t.generation == handler.generation)
        else {
            return;
        };
        if !unsafe {
            jsapi::JS_GetElement(
                cx,
                table.functions.handle().into(),
                handler.index,
                out.into(),
            )
        } {
            unsafe { jsapi::JS_ClearPendingException(cx) };
        }
    });
}

impl Drop for ScriptHost {
    fn drop(&mut self) {
        // Module and handler roots must be released while the runtime is still alive.
        STATE.with(|s| s.borrow_mut().take());
        value::clear_atom_cache();
        info!("Script runtime shut down");
    }
}

// ---------------------------------------------------------------------------
// Promise job queue
// ---------------------------------------------------------------------------

static JOB_QUEUE_TRAPS: mozjs::glue::JobQueueTraps = mozjs::glue::JobQueueTraps {
    getHostDefinedData: Some(job_queue_host_defined_data),
    getHostDefinedGlobal: Some(job_queue_host_defined_global),
    runJobs: Some(job_queue_run_jobs),
    traceNonGCThingMicroTask: Some(job_queue_trace_non_gc),
};

unsafe extern "C" fn job_queue_host_defined_data(
    cx: *mut RawJSContext,
    incumbent_global: jsapi::MutableHandle<*mut JSObject>,
    host_defined_data: jsapi::MutableHandle<*mut JSObject>,
) -> bool {
    unsafe {
        incumbent_global.set(jsapi::CurrentGlobalOrNull(cx));
        host_defined_data.set(ptr::null_mut());
    }
    true
}

unsafe extern "C" fn job_queue_host_defined_global(
    cx: *mut RawJSContext,
    data: jsapi::MutableHandle<*mut JSObject>,
) -> bool {
    unsafe { data.set(jsapi::CurrentGlobalOrNull(cx)) };
    true
}

unsafe extern "C" fn job_queue_run_jobs(cx: *mut RawJSContext) {
    unsafe { run_jobs(cx) };
}

const unsafe extern "C" fn job_queue_trace_non_gc(_trc: *mut jsapi::JSTracer, _value: *mut Value) {}

/// Drains the microtask queue. Errors thrown by individual jobs are logged so
/// one failing promise reaction cannot stall the rest of the queue.
unsafe fn run_jobs(cx: *mut RawJSContext) {
    unsafe {
        loop {
            rooted!(in(cx) let mut task = NullValue());
            mozjs::glue::JS_DequeueNextMicroTask(cx, task.handle_mut().into());
            if task.get().is_null() {
                break;
            }
            if !jsapi::IsJSMicroTask(&task.get()) {
                continue;
            }
            rooted!(in(cx) let job = jsapi::ToUnwrappedJSMicroTask(&task.get()));
            if job.get().is_null() || jsapi::GetExecutionGlobalFromJSMicroTask(job.get()).is_null()
            {
                continue;
            }
            if !jsapi::RunJSMicroTask(cx, job.handle().into()) {
                error!("Uncaught error in promise job: {}", pending_exception(cx));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Module loading
// ---------------------------------------------------------------------------

/// Compiles a module. On a syntax error the exception is left pending so the
/// module loader can propagate the original `SyntaxError`.
unsafe fn compile_module(cx: *mut RawJSContext, id: &str, source: &str) -> Result<*mut JSObject> {
    unsafe {
        let safe_cx = JSContext::from_ptr(NonNull::new_unchecked(cx));
        let options = CompileOptionsWrapper::new(&safe_cx, CString::new(id)?, 1);
        let mut text = transform_str_to_source_text(source);
        let module = jsapi::CompileModule1(cx, options.ptr, &raw mut text);
        if module.is_null() {
            bail!("failed to compile module '{id}'");
        }
        rooted!(in(cx) let module = module);
        rooted!(in(cx) let mut private = UndefinedValue());
        let mut safe_cx = JSContext::from_ptr(NonNull::new_unchecked(cx));
        id.to_jsval(&mut safe_cx, private.handle_mut());
        jsapi::SetModulePrivate(module.get(), &private.get());
        Ok(module.get())
    }
}

/// True for the engine's built-in modules (`deflorta`, `deflorta/ui`, …).
pub fn is_builtin_module(specifier: &str) -> bool {
    BUILTIN_MODULES.iter().any(|(name, _)| *name == specifier)
}

/// Resolves an import specifier to a module id: either a builtin name or a
/// normalized path relative to the game directory. Specifiers name files
/// exactly; no extensions are added.
pub fn resolve_specifier(referrer: &str, specifier: &str) -> Result<String> {
    if is_builtin_module(specifier) {
        return Ok(specifier.to_owned());
    }
    let base = if let Some(rest) = specifier.strip_prefix('/') {
        PathBuf::from(rest)
    } else if specifier.starts_with("./") || specifier.starts_with("../") {
        if is_builtin_module(referrer) {
            bail!("builtin module '{referrer}' cannot import '{specifier}'");
        }
        Path::new(referrer)
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .join(specifier)
    } else {
        bail!("unknown module '{specifier}' (game modules must start with './', '../' or '/')");
    };
    normalize_game_path(&base)
        .ok_or_else(|| anyhow!("module path '{specifier}' escapes the game directory"))
}

fn module_source(id: &str) -> Result<String> {
    if let Some((_, src)) = BUILTIN_MODULES.iter().find(|(name, _)| *name == id) {
        return Ok((*src).to_owned());
    }
    let files = with_state(|s| s.files.clone());
    files
        .read_to_string(id)
        .map_err(|e| anyhow!("cannot read module '{id}': {e}"))
}

unsafe extern "C" fn module_load_hook(
    cx: *mut RawJSContext,
    referrer: jsapi::Handle<*mut JSScript>,
    request: jsapi::Handle<*mut JSObject>,
    _host_defined: jsapi::Handle<Value>,
    payload: jsapi::Handle<Value>,
    _line: u32,
    _column: ColumnNumberOneOrigin,
) -> bool {
    unsafe {
        let referrer_id = if referrer.get().is_null() {
            String::new()
        } else {
            rooted!(in(cx) let mut private = UndefinedValue());
            mozjs::glue::JS_GetScriptPrivate(referrer.get(), private.handle_mut().into());
            if private.get().is_string() {
                value_to_string(cx, private.handle())
            } else {
                String::new()
            }
        };
        let specifier = jsapi::GetModuleRequestSpecifier(cx, request);
        let Some(specifier) = NonNull::new(specifier) else {
            return false;
        };
        let specifier =
            jsstr_to_string(&JSContext::from_ptr(NonNull::new_unchecked(cx)), specifier);

        let module = match load_module(cx, &referrer_id, &specifier) {
            Ok(module) => module,
            Err(err) => {
                if !jsapi::JS_IsExceptionPending(cx) {
                    throw_error(cx, &format!("{err}"));
                }
                return false;
            }
        };
        rooted!(in(cx) let module = module);
        jsapi::FinishLoadingImportedModule(
            cx,
            referrer,
            request,
            payload,
            module.handle().into(),
            false,
        )
    }
}

unsafe fn load_module(
    cx: *mut RawJSContext,
    referrer: &str,
    specifier: &str,
) -> Result<*mut JSObject> {
    let id = resolve_specifier(referrer, specifier)?;
    if let Some(module) = with_state(|s| s.modules.get(&id).map(|m| m.get())) {
        return Ok(module);
    }
    let source = module_source(&id)?;
    debug!("Compiling module '{id}' ({} bytes)", source.len());
    let module = unsafe { compile_module(cx, &id, &source)? };
    with_state(|s| {
        s.modules
            .insert(id, RootedTraceableBox::from_box(Heap::boxed(module)))
    });
    Ok(module)
}

const unsafe extern "C" fn on_modules_loaded(
    _cx: *mut RawJSContext,
    _host_defined: jsapi::Handle<Value>,
) -> bool {
    true
}

unsafe extern "C" fn on_modules_failed(
    cx: *mut RawJSContext,
    _host_defined: jsapi::Handle<Value>,
    error: jsapi::Handle<Value>,
) -> bool {
    let message = unsafe { describe_error(cx, Handle::from_raw(error)) };
    with_state(|s| s.load_error = Some(message));
    true
}

// ---------------------------------------------------------------------------
// Error helpers
// ---------------------------------------------------------------------------

unsafe fn throw_error(cx: *mut RawJSContext, message: &str) {
    let message = CString::new(message.replace('\0', " ")).unwrap_or_default();
    unsafe { mozjs::glue::ReportErrorUTF8(cx, message.as_ptr()) };
}

unsafe fn pending_exception(cx: *mut RawJSContext) -> anyhow::Error {
    unsafe {
        rooted!(in(cx) let mut exception = UndefinedValue());
        if !jsapi::JS_GetPendingException(cx, exception.handle_mut().into()) {
            return anyhow!("unknown JavaScript error (uncatchable exception)");
        }
        jsapi::JS_ClearPendingException(cx);
        anyhow!("{}", describe_error(cx, exception.handle()))
    }
}

unsafe fn value_to_string(cx: *mut RawJSContext, value: Handle<Value>) -> String {
    let mut safe_cx = unsafe { JSContext::from_ptr(NonNull::new_unchecked(cx)) };
    if let Ok(ConversionResult::Success(s)) = String::from_jsval(&mut safe_cx, value, ()) {
        s
    } else {
        unsafe { jsapi::JS_ClearPendingException(cx) };
        "<unprintable value>".to_owned()
    }
}

unsafe fn get_property_string(
    cx: *mut RawJSContext,
    obj: *mut JSObject,
    name: &std::ffi::CStr,
) -> Option<String> {
    unsafe {
        rooted!(in(cx) let obj = obj);
        rooted!(in(cx) let mut value = UndefinedValue());
        if !jsapi::JS_GetProperty(
            cx,
            obj.handle().into(),
            name.as_ptr(),
            value.handle_mut().into(),
        ) {
            jsapi::JS_ClearPendingException(cx);
            return None;
        }
        if value.get().is_undefined() || value.get().is_null() {
            return None;
        }
        Some(value_to_string(cx, value.handle()))
    }
}

/// Formats a thrown value with its location and stack when available.
unsafe fn describe_error(cx: *mut RawJSContext, value: Handle<Value>) -> String {
    unsafe {
        let mut text = value_to_string(cx, value);
        if value.get().is_object() {
            let obj = value.get().to_object();
            let file = get_property_string(cx, obj, c"fileName");
            let line = get_property_string(cx, obj, c"lineNumber");
            if let (Some(file), Some(line)) = (file, line)
                && !file.is_empty()
                && line != "0"
            {
                let _ = write!(text, "\n    at {file}:{line}");
            }
            if let Some(stack) =
                get_property_string(cx, obj, c"stack").filter(|s| !s.trim().is_empty())
            {
                text.push_str("\nstack:\n");
                text.push_str(stack.trim_end());
            }
        }
        text
    }
}
