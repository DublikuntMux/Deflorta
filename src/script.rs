//! `SpiderMonkey` host: ES module loading, the native `__host` API and the
//! dispatch/pump bridge between Rust and the JavaScript runtime.

use std::cell::RefCell;
use std::collections::HashMap;
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
    self, CallArgs, ColumnNumberOneOrigin, Heap, JSObject, JSScript, OnNewGlobalHookOption,
    PromiseState, Value,
};
use mozjs::jsval::{BooleanValue, NullValue, UndefinedValue};
use mozjs::rooted;
use mozjs::rust::{
    CompileOptionsWrapper, JSEngine, RealmOptions, Runtime, SIMPLE_GLOBAL_CLASS,
    transform_str_to_source_text,
};
use num_traits::ToPrimitive;

use crate::assets::normalize_game_path;

/// Built-in modules, importable by bare specifier.
const BUILTIN_MODULES: &[(&str, &str)] = &[
    ("deflorta", include_str!("../runtime/deflorta.js")),
    ("deflorta/core", include_str!("../runtime/core.js")),
    ("deflorta/ui", include_str!("../runtime/ui.js")),
    ("deflorta/text", include_str!("../runtime/text.js")),
    ("deflorta/scene", include_str!("../runtime/scene.js")),
    ("deflorta/story", include_str!("../runtime/story.js")),
    ("deflorta/screens", include_str!("../runtime/screens.js")),
];

const BOOT_MODULE: &str = "import \"deflorta\";\nimport \"./main.js\";\n";

/// Per-thread state shared with native callbacks and the module loader hook.
struct HostState {
    game_dir: PathBuf,
    data_dir: Option<PathBuf>,
    modules: HashMap<String, RootedTraceableBox<Heap<*mut JSObject>>>,
    load_error: Option<String>,
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
    pub fn new(game_dir: PathBuf) -> Result<Self> {
        debug!("Initializing SpiderMonkey");
        let engine = JSEngine::init().map_err(|e| anyhow!("failed to init SpiderMonkey: {e:?}"))?;
        let mut runtime = Runtime::new(engine.handle());
        let cx = unsafe { runtime.cx().raw_cx() };

        STATE.with(|s| {
            *s.borrow_mut() = Some(HostState {
                game_dir,
                data_dir: None,
                modules: HashMap::new(),
                load_error: None,
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
            define_host_object(cx, global.get())?;
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

    /// Delivers an input/timer event to the JS runtime, then runs promise jobs
    /// so story code can advance to its next suspension point.
    pub fn dispatch(&mut self, event_json: &str) -> Result<()> {
        self.call_global("__deflorta_dispatch", Some(event_json))?;
        let cx = self.cx();
        unsafe { run_jobs(cx) };
        Ok(())
    }

    /// Collects pending output (UI tree, commands) from the JS runtime.
    pub fn pump(&mut self) -> Result<Option<String>> {
        let out = self.call_global("__deflorta_pump", None)?;
        Ok(out.filter(|s| !s.is_empty()))
    }

    pub fn maybe_gc(&mut self) {
        let cx = self.cx();
        unsafe { jsapi::JS_MaybeGC(cx) };
    }

    pub fn set_data_dir(dir: PathBuf) {
        info!("User data directory: {}", dir.display());
        with_state(|s| s.data_dir = Some(dir));
    }

    fn call_global(&mut self, name: &str, arg: Option<&str>) -> Result<Option<String>> {
        let global = self.global.get();
        let cx = self.cx();
        let name = CString::new(name)?;
        unsafe {
            rooted!(in(cx) let global = global);
            rooted!(in(cx) let mut argv = UndefinedValue());
            rooted!(in(cx) let mut rval = UndefinedValue());
            let mut safe_cx = JSContext::from_ptr(NonNull::new_unchecked(cx));
            let args = arg.map_or_else(jsapi::HandleValueArray::empty, |arg| {
                arg.to_jsval(&mut safe_cx, argv.handle_mut());
                let raw: jsapi::HandleValue = argv.handle().into();
                jsapi::HandleValueArray::from(raw)
            });
            if !jsapi::JS_CallFunctionName(
                cx,
                global.handle().into(),
                name.as_ptr(),
                &raw const args,
                rval.handle_mut().into(),
            ) {
                return Err(pending_exception(cx));
            }
            if rval.get().is_string() {
                Ok(Some(value_to_string(cx, rval.handle())))
            } else {
                Ok(None)
            }
        }
    }
}

impl Drop for ScriptHost {
    fn drop(&mut self) {
        // Module roots must be released while the runtime is still alive.
        STATE.with(|s| s.borrow_mut().take());
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

/// Resolves an import specifier to a module id: either a builtin name or a
/// normalized path relative to the game directory.
fn resolve_specifier(referrer: &str, specifier: &str) -> Result<String> {
    if BUILTIN_MODULES.iter().any(|(name, _)| *name == specifier) {
        return Ok(specifier.to_owned());
    }
    let base = if let Some(rest) = specifier.strip_prefix('/') {
        PathBuf::from(rest)
    } else if specifier.starts_with("./") || specifier.starts_with("../") {
        if BUILTIN_MODULES.iter().any(|(name, _)| *name == referrer) {
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
    let path = with_state(|s| s.game_dir.join(id));
    std::fs::read_to_string(&path)
        .map_err(|e| anyhow!("cannot read module '{}': {e}", path.display()))
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

// ---------------------------------------------------------------------------
// Native `__host` object
// ---------------------------------------------------------------------------

type Native = unsafe extern "C" fn(*mut RawJSContext, u32, *mut Value) -> bool;

unsafe fn define_host_object(cx: *mut RawJSContext, global: *mut JSObject) -> Result<()> {
    const NATIVES: &[(&std::ffi::CStr, Native, u32)] = &[
        (c"log", host_log, 2),
        (c"readText", host_read_text, 1),
        (c"readData", host_read_data, 1),
        (c"writeData", host_write_data, 2),
        (c"deleteData", host_delete_data, 1),
        (c"listData", host_list_data, 0),
    ];
    unsafe {
        rooted!(in(cx) let global = global);
        rooted!(in(cx) let host = jsapi::JS_NewPlainObject(cx));
        if host.get().is_null() {
            bail!("failed to create __host");
        }
        for (name, native, nargs) in NATIVES {
            let f = jsapi::JS_DefineFunction(
                cx,
                host.handle().into(),
                name.as_ptr(),
                Some(*native),
                *nargs,
                0,
            );
            if f.is_null() {
                bail!("failed to define __host.{}", name.to_string_lossy());
            }
        }
        rooted!(in(cx) let host_value = mozjs::jsval::ObjectValue(host.get()));
        if !jsapi::JS_DefineProperty(
            cx,
            global.handle().into(),
            c"__host".as_ptr(),
            host_value.handle().into(),
            0,
        ) {
            bail!("failed to define __host");
        }
    }
    Ok(())
}

unsafe fn arg_string(cx: *mut RawJSContext, args: &CallArgs, index: u32) -> Option<String> {
    if index >= args.argc_ {
        return None;
    }
    let value = unsafe { Handle::from_raw(args.get(index)) };
    if value.get().is_undefined() || value.get().is_null() {
        return None;
    }
    Some(unsafe { value_to_string(cx, value) })
}

unsafe fn return_string(cx: *mut RawJSContext, args: &CallArgs, value: Option<&str>) {
    unsafe {
        let mut rval = MutableHandle::from_raw(args.rval());
        match value {
            Some(s) => {
                let mut safe_cx = JSContext::from_ptr(NonNull::new_unchecked(cx));
                s.to_jsval(&mut safe_cx, rval);
            }
            None => rval.set(NullValue()),
        }
    }
}

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

unsafe extern "C" fn host_log(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    unsafe {
        let arguments = CallArgs::from_vp(vp, argc);
        let level = arg_string(cx, &arguments, 0).unwrap_or_default();
        let message = arg_string(cx, &arguments, 1).unwrap_or_default();
        let level = match level.as_str() {
            "error" => log::Level::Error,
            "warn" => log::Level::Warn,
            "debug" => log::Level::Debug,
            "trace" => log::Level::Trace,
            _ => log::Level::Info,
        };
        log::log!(target: "deflorta::js", level, "{message}");
        arguments.rval().set(UndefinedValue());
        true
    }
}

unsafe extern "C" fn host_read_text(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    unsafe {
        let arguments = CallArgs::from_vp(vp, argc);
        let text = arg_string(cx, &arguments, 0)
            .and_then(|p| normalize_game_path(Path::new(&p)))
            .and_then(|p| std::fs::read_to_string(with_state(|s| s.game_dir.join(p))).ok());
        return_string(cx, &arguments, text.as_deref());
        true
    }
}

unsafe extern "C" fn host_read_data(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    unsafe {
        let arguments = CallArgs::from_vp(vp, argc);
        let text = arg_string(cx, &arguments, 0)
            .and_then(|name| data_path(&name))
            .and_then(|path| std::fs::read_to_string(path).ok());
        return_string(cx, &arguments, text.as_deref());
        true
    }
}

unsafe extern "C" fn host_write_data(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    unsafe {
        let arguments = CallArgs::from_vp(vp, argc);
        let (Some(path), Some(text)) = (
            arg_string(cx, &arguments, 0).and_then(|n| data_path(&n)),
            arg_string(cx, &arguments, 1),
        ) else {
            throw_error(
                cx,
                "writeData: invalid name or data directory not configured",
            );
            return false;
        };
        // Write to a temporary file first so a crash never leaves a torn save.
        let tmp = path.with_extension("json.tmp");
        let result = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&tmp, text))
            .and_then(|()| std::fs::rename(&tmp, &path));
        if let Err(err) = result {
            throw_error(cx, &format!("writeData failed: {err}"));
            return false;
        }
        arguments.rval().set(BooleanValue(true));
        true
    }
}

unsafe extern "C" fn host_delete_data(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    unsafe {
        let arguments = CallArgs::from_vp(vp, argc);
        let deleted = arg_string(cx, &arguments, 0)
            .and_then(|name| data_path(&name))
            .is_some_and(|path| std::fs::remove_file(path).is_ok());
        arguments.rval().set(BooleanValue(deleted));
        true
    }
}

/// Returns a JSON array of `{name, modified}` for every stored data file.
unsafe extern "C" fn host_list_data(cx: *mut RawJSContext, argc: u32, vp: *mut Value) -> bool {
    unsafe {
        let arguments = CallArgs::from_vp(vp, argc);
        let mut entries = Vec::new();
        if let Some(dir) = with_state(|s| s.data_dir.clone())
            && let Ok(read) = std::fs::read_dir(dir)
        {
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
                entries.push(serde_json::json!({ "name": name, "modified": modified }));
            }
        }
        let json = serde_json::Value::Array(entries).to_string();
        return_string(cx, &arguments, Some(&json));
        true
    }
}
