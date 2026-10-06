// deflorta/core — native modules, timers, config, storage and the event loop glue.
//
// The engine exposes typed native modules (`native.audio`, `native.ui`, …) that
// JS calls synchronously with plain values; nothing is serialized. The engine
// calls back into two functions registered with `native.connect`:
//   dispatch(event)  delivers one input/timer event object
//   flush()          runs at the end of each turn to commit pending output

/** Native engine modules: files, storage, timers, app, audio, ui. */
export const native = globalThis.__native;
delete globalThis.__native;

// ---------------------------------------------------------------------------
// Logging
// ---------------------------------------------------------------------------

function format(value) {
  if (typeof value === "string") return value;
  if (value instanceof Error) return `${value}\n${value.stack ?? ""}`.trimEnd();
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

function logger(level) {
  return (...args) => native.log(level, args.map(format).join(" "));
}

export const log = logger("info");

globalThis.console = {
  log: logger("info"),
  info: logger("info"),
  debug: logger("debug"),
  warn: logger("warn"),
  error: logger("error"),
};

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

export const config = {
  /** Directory name for saves; must be unique per game. */
  id: "deflorta-game",
  title: "Deflorta",
  /** Virtual resolution. Everything is laid out in these units and scaled to the window. */
  width: 1280,
  height: 720,
  /** Default font family; fonts are loaded from the game's fonts/ directory. */
  font: "Noto Sans",
  /** Characters per second for dialogue. 0 shows text instantly. */
  textSpeed: 40,
  /** Delay between lines while skipping, in milliseconds. */
  skipDelay: 50,
  clearColor: "#000000",
};

/** Updates game configuration. Call at the top level of main.js. */
export function configure(options) {
  Object.assign(config, options);
  native.app.configure(config);
}

// ---------------------------------------------------------------------------
// Timers
// ---------------------------------------------------------------------------

let timerSeq = 0;
const timers = new Map();

/** Calls `fn` after `ms` milliseconds. Returns an id for clearTimer. */
export function setTimer(ms, fn) {
  const id = ++timerSeq;
  timers.set(id, fn);
  native.timers.set(id, Math.max(0, Number(ms) || 0));
  return id;
}

export function clearTimer(id) {
  if (timers.delete(id)) native.timers.clear(id);
}

globalThis.setTimeout = (fn, ms = 0, ...args) =>
  setTimer(ms, () => fn(...args));
globalThis.clearTimeout = clearTimer;

// ---------------------------------------------------------------------------
// Storage (saves, preferences). Values are JSON-serialized.
// ---------------------------------------------------------------------------

export const storage = {
  read(name) {
    const text = native.storage.read(name);
    if (text == null) return null;
    try {
      return JSON.parse(text);
    } catch (e) {
      console.warn(`storage: '${name}' is corrupt: ${e}`);
      return null;
    }
  },
  write(name, value) {
    native.storage.write(name, JSON.stringify(value));
  },
  remove(name) {
    return native.storage.remove(name);
  },
  /** Returns [{ name, modified }] for every stored entry. */
  list() {
    return native.storage.list();
  },
};

/** Reads a text file from the game directory, or null if it does not exist. */
export function readText(path) {
  return native.files.readText(path);
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

const listeners = new Map();

/** Subscribes to an engine event ("key", "click", "wheel", "revealed", "boot", "quit", "error"). */
export function on(type, fn) {
  if (!listeners.has(type)) listeners.set(type, []);
  listeners.get(type).push(fn);
  return () => {
    const list = listeners.get(type);
    const i = list.indexOf(fn);
    if (i >= 0) list.splice(i, 1);
  };
}

export function emit(type, event) {
  for (const fn of [...(listeners.get(type) ?? [])]) {
    try {
      if (fn(event) === true) return true;
    } catch (e) {
      reportError(e);
    }
  }
  return false;
}

/** Logs an error and notifies "error" listeners (the default error screen). */
export function reportError(error) {
  console.error(error);
  if (!listeners.get("error")?.length) return;
  for (const fn of listeners.get("error")) {
    try {
      fn(error);
    } catch (e) {
      console.error(e);
    }
  }
}

// ---------------------------------------------------------------------------
// Engine bridge
// ---------------------------------------------------------------------------

const flushHooks = [];

/** Registers a function run at the end of every turn to commit output (UI tree, music). */
export function onFlush(fn) {
  flushHooks.push(fn);
}

function dispatch(event) {
  try {
    if (event.type === "timer") {
      const fn = timers.get(event.id);
      timers.delete(event.id);
      fn?.();
    } else {
      emit(event.type, event);
    }
  } catch (e) {
    reportError(e);
  }
}

function flush() {
  for (const fn of flushHooks) {
    try {
      fn();
    } catch (e) {
      reportError(e);
    }
  }
}

native.connect(dispatch, flush);
native.app.configure(config);
