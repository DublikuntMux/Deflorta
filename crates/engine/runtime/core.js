export const native = globalThis.__native;
delete globalThis.__native;

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

export const config = {
  id: "deflorta-game",
  title: "Deflorta",
  width: 1280,
  height: 720,
  font: "Noto Sans",
  textSpeed: 40,
  skipDelay: 50,
  clearColor: "#000000",
};

export function configure(options) {
  Object.assign(config, options);
  native.app.configure(config);
}

let timerSeq = 0;
const timers = new Map();

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
  list() {
    return native.storage.list();
  },
};

export function readText(path) {
  return native.files.readText(path);
}

const listeners = new Map();

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

const flushHooks = [];

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
