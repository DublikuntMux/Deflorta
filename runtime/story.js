// deflorta/story — labels, dialogue, choices, scene state, saves and rollback.
//
// Story code is ordinary async JavaScript. Every interaction (say, menu,
// pause, input) is a numbered *checkpoint*. Saving records the state at the
// start of the current label (the *root*) plus the inputs given at each
// checkpoint since then. Loading or rolling back restores the root state
// and re-runs the label, resolving checkpoints instantly until the target
// is reached. Story code must therefore be deterministic: keep game state in
// `store`, use `random()` instead of Math.random(), and only await engine
// functions.

import { addFrameSource, clearTimer, command, config, on, reportError, setTimer, storage } from "deflorta/core";
import {
  FILL,
  box,
  exitWith,
  hideScreen,
  img,
  invalidate,
  isShown,
  markInstant,
  replaceScreens,
  setSceneLayer,
  showScreen,
  shownScreens,
} from "deflorta/ui";

// ---------------------------------------------------------------------------
// Game state
// ---------------------------------------------------------------------------

/** Game variables. Must stay JSON-serializable; it is saved and rolled back. */
export const store = {};
const storeDefaults = {};

/** Declares default store values, applied when a new game starts. */
export function defaults(values) {
  Object.assign(storeDefaults, structuredCloneJson(values));
  for (const [k, v] of Object.entries(values)) if (!(k in store)) store[k] = structuredCloneJson(v);
}

function structuredCloneJson(value) {
  return value === undefined ? undefined : JSON.parse(JSON.stringify(value));
}

function replaceContents(target, source) {
  for (const k of Object.keys(target)) delete target[k];
  Object.assign(target, source);
}

/** Preferences persist across games and are not rolled back. */
export const prefs = {
  textSpeed: null,
  musicVolume: 0.8,
  soundVolume: 0.8,
  fullscreen: false,
  autoForward: false,
  autoDelay: 1.5,
};

export function savePrefs() {
  storage.write("prefs", prefs);
  applyPrefs();
}

function applyPrefs() {
  command("volume", { channel: "music", value: prefs.musicVolume });
  command("volume", { channel: "sound", value: prefs.soundVolume });
  command("fullscreen", { on: prefs.fullscreen });
}

export function textSpeed() {
  return prefs.textSpeed ?? config.textSpeed;
}

// Deterministic random numbers (mulberry32), part of the saved state.
let rngState = 1;

/** Returns a random number in [0, 1) that replays identically after load/rollback. */
export function random() {
  rngState = (rngState + 0x6d2b79f5) | 0;
  let t = rngState;
  t = Math.imul(t ^ (t >>> 15), t | 1);
  t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}

/** Returns a random integer in [min, max]. */
export function randInt(min, max) {
  return min + Math.floor(random() * (max - min + 1));
}

// ---------------------------------------------------------------------------
// Images, positions and transitions
// ---------------------------------------------------------------------------

const images = new Map();

/**
 * Declares an image. The first word of the name is its tag: showing
 * "eileen happy" replaces any shown "eileen ..." image.
 * Undeclared names resolve to `images/<name>.png`.
 */
export function image(name, src, options = {}) {
  images.set(name, { src, ...options });
}

function imageSpec(name) {
  return images.get(name) ?? { src: `images/${name}.png` };
}

const tagOf = (name) => name.split(" ")[0];

/** Positions: xalign/yalign place the image's anchor point relative to the screen. */
export const left = { xalign: 0.2, yalign: 1 };
export const center = { xalign: 0.5, yalign: 1 };
export const right = { xalign: 0.8, yalign: 1 };
export const truecenter = { xalign: 0.5, yalign: 0.5 };
export const at = (xalign, yalign = 1, extra = {}) => ({ xalign, yalign, ...extra });

/**
 * Transitions describe how elements enter and leave.
 * `in` gives the starting values, `out` the final values (opacity, x, y, scale).
 */
export const dissolve = (dur = 0.5) => ({ dur, in: { opacity: 0 }, out: { opacity: 0 } });
export const fade = dissolve;
export const moveinleft = (dur = 0.5) => ({ dur, in: { x: -400, opacity: 0 }, out: { opacity: 0 } });
export const moveinright = (dur = 0.5) => ({ dur, in: { x: 400, opacity: 0 }, out: { opacity: 0 } });
export const moveoutleft = (dur = 0.5) => ({ dur, in: { opacity: 0 }, out: { x: -400, opacity: 0 } });
export const moveoutright = (dur = 0.5) => ({ dur, in: { opacity: 0 }, out: { x: 400, opacity: 0 } });
export const zoomin = (dur = 0.5) => ({ dur, in: { scale: 0.6, opacity: 0 }, out: { scale: 0.6, opacity: 0 } });

const enterSpec = (t) => t && { dur: t.dur, ...t.in };
const exitSpec = (t, hold = false) => t && (hold ? { dur: t.dur, opacity: 1 } : { dur: t.dur, ...t.out });

// ---------------------------------------------------------------------------
// Scene
// ---------------------------------------------------------------------------

// Serializable scene state; saved and rolled back with the store.
const scene = { bg: null, sprites: [], music: null };
const pendingEnters = new Map();
let musicFade = { fadeIn: 0, fadeOut: 0 };
let sentMusic = "null";

function replaying() {
  return run.target >= 0;
}

/** Clears the screen and optionally shows a background. Hides the dialogue window. */
export function scene_(name = null, options = {}) {
  const t = options.with;
  if (scene.bg) exitWith(`bg:${scene.bg}`, exitSpec(t, true));
  for (const s of scene.sprites) exitWith(`sprite:${s.tag}:${s.name}`, exitSpec(t));
  scene.sprites = [];
  scene.bg = name;
  if (name) pendingEnters.set(`bg:${name}`, enterSpec(t));
  hideScreen("say");
  invalidate();
}
export { scene_ as scene };

/** Shows an image, replacing any image with the same tag. */
export function show(name, options = {}) {
  const tag = tagOf(name);
  const t = options.with;
  const index = scene.sprites.findIndex((s) => s.tag === tag);
  const previous = scene.sprites[index];
  const entry = { tag, name, at: options.at ?? previous?.at ?? center, zorder: options.zorder ?? previous?.zorder ?? 0 };
  if (previous && previous.name !== name) exitWith(`sprite:${tag}:${previous.name}`, exitSpec(t));
  if (!previous || previous.name !== name) pendingEnters.set(`sprite:${tag}:${name}`, enterSpec(t));
  if (index >= 0) scene.sprites[index] = entry;
  else scene.sprites.push(entry);
  scene.sprites.sort((a, b) => a.zorder - b.zorder);
  invalidate();
}

/** Hides the image with the given tag (or full name). */
export function hide(name, options = {}) {
  const tag = tagOf(name);
  const previous = scene.sprites.find((s) => s.tag === tag);
  if (!previous) return;
  exitWith(`sprite:${tag}:${previous.name}`, exitSpec(options.with));
  scene.sprites = scene.sprites.filter((s) => s.tag !== tag);
  invalidate();
}

function renderScene() {
  const children = [];
  if (scene.bg) {
    const key = `bg:${scene.bg}`;
    children.push(img(imageSpec(scene.bg).src, { key, fit: "cover", style: FILL, enter: pendingEnters.get(key) }));
  }
  for (const s of scene.sprites) {
    const key = `sprite:${s.tag}:${s.name}`;
    const spec = imageSpec(s.name);
    children.push(
      img(spec.src, {
        key,
        anchor: [s.at.xalign, s.at.yalign],
        style: {
          position: "absolute",
          left: `${s.at.xalign * 100}%`,
          top: `${s.at.yalign * 100}%`,
          scale: (s.at.zoom ?? 1) * (spec.zoom ?? 1),
        },
        enter: pendingEnters.get(key),
      }),
    );
  }
  return box({ key: "scene", style: FILL }, children);
}

setSceneLayer(renderScene);

addFrameSource(() => {
  pendingEnters.clear();
  const music = JSON.stringify(scene.music);
  if (music === sentMusic) return;
  sentMusic = music;
  command("music", { ...(scene.music ?? { file: null }), ...musicFade });
  musicFade = { fadeIn: 0, fadeOut: 0 };
});

/** Background music. State is part of the scene, so it survives save/load. */
export const music = {
  play(file, { loop = true, fadeIn = 0, fadeOut = 0.5, volume = 1 } = {}) {
    scene.music = { file, loop, volume };
    musicFade = { fadeIn, fadeOut };
  },
  stop({ fadeOut = 0.5 } = {}) {
    scene.music = null;
    musicFade = { fadeIn: 0, fadeOut };
  },
};

/** One-shot sound effects. Skipped while fast-forwarding a load or rollback. */
export const sound = {
  play(file, { volume = 1 } = {}) {
    if (!replaying()) command("sound", { file, volume });
  },
};

// ---------------------------------------------------------------------------
// Labels and the story runner
// ---------------------------------------------------------------------------

const labels = new Map();

/** Declares a label: a named async function that story flow can jump to. */
export function label(name, fn) {
  labels.set(name, fn);
}

class Jump {
  constructor(target) {
    this.target = target;
  }
}

/** Transfers control to another label. Ends the current label (and any calls). */
export function jump(name) {
  throw new Jump(name);
}

/** Runs another label and returns to the caller when it finishes. */
export async function call(name, ...args) {
  const fn = labels.get(name);
  if (!fn) throw new Error(`unknown label '${name}'`);
  return fn(...args);
}

const MAX_HISTORY = 64;

const run = {
  gen: 0,
  root: null, // { label, snapshot } at the start of the current label
  count: 0, // checkpoints reached since the root
  inputs: {}, // checkpoint index -> recorded input
  stops: [], // checkpoint index -> true if rollback may stop there
  target: -1, // when >= 0, fast-forward until this checkpoint
  pending: null, // the checkpoint waiting for the player
  history: [], // earlier roots, for rolling back across jumps
};

// Engine screens are managed by the runtime; every other shown screen is game
// state and is saved, restored and rolled back with the scene.
const SYSTEM_SCREENS = new Set(["say", "choice", "game_menu", "main_menu", "notify", "error"]);
const isGameScreen = (name) => !SYSTEM_SCREENS.has(name);

function snapshot() {
  return JSON.stringify({ store, scene, screens: shownScreens(isGameScreen), rng: rngState });
}

function restore(snap) {
  const data = JSON.parse(snap);
  replaceContents(store, data.store);
  scene.bg = data.scene.bg;
  scene.sprites = data.scene.sprites;
  scene.music = data.scene.music;
  replaceScreens(data.screens ?? [], isGameScreen);
  rngState = data.rng;
}

function beginRoot(name) {
  if (run.root) {
    run.history.push({ root: run.root, inputs: run.inputs, stops: run.stops });
    if (run.history.length > MAX_HISTORY) run.history.shift();
  }
  run.root = { label: name, snapshot: snapshot() };
  run.count = 0;
  run.inputs = {};
  run.stops = [];
}

async function runStory(start, resume = false) {
  const gen = ++run.gen;
  let name = start;
  while (name != null) {
    if (!resume) beginRoot(name);
    resume = false;
    const fn = labels.get(name);
    if (!fn) throw new Error(`unknown label '${name}'`);
    try {
      await fn();
      name = null;
    } catch (e) {
      if (run.gen !== gen) return;
      if (!(e instanceof Jump)) throw e;
      name = e.target;
    }
  }
  if (run.gen === gen) endGame();
}

function startRun(start, resume) {
  runStory(start, resume).catch((e) => {
    reportError(e);
  });
}

/** True while a game is in progress. */
export function inGame() {
  return run.root !== null;
}

/**
 * Suspends the story until the player responds. `present` shows the UI and
 * receives the pending checkpoint; call `pending.resolve(value)` to continue.
 */
export function checkpoint(kind, present, { record = false, rollback = true } = {}) {
  const index = run.count++;
  run.stops[index] = rollback;
  if (index < run.target) return Promise.resolve(record ? run.inputs[index] : undefined);
  if (index === run.target) {
    run.target = -1;
    markInstant();
  }
  const gen = run.gen;
  return new Promise((resolve) => {
    const pending = {
      kind,
      index,
      cleanup: null,
      resolve(value) {
        if (run.gen !== gen || run.pending !== pending) return;
        run.pending = null;
        pending.cleanup?.();
        if (record) run.inputs[index] = value;
        resolve(value);
      },
    };
    run.pending = pending;
    present(pending);
  });
}

// ---------------------------------------------------------------------------
// Dialogue, choices, pauses
// ---------------------------------------------------------------------------

let skipping = false;
let currentLine = "";

/** Creates a speaking character. Call it with text: `await eileen("Hi!")` or eileen`Hi!`. */
export function character(name, options = {}) {
  const who = { name, color: "#ffffff", ...options };
  const speak = (first, ...rest) =>
    Array.isArray(first) && first.raw ? say(who, String.raw(first, ...rest)) : say(who, first, rest[0]);
  speak.who = who;
  return speak;
}

/** Shows a line of dialogue and waits for the player. `say("text")` narrates. */
export function say(who, what, options = {}) {
  if (what === undefined) {
    what = who;
    who = null;
  }
  if (typeof who === "string") who = { name: who };
  return checkpoint("say", (pending) => {
    currentLine = who ? `${who.name}: ${what}` : String(what);
    showScreen("say", { who, what: String(what), cps: textSpeed(), ...options });
    if (skipping) pending.cleanup = cancelOnResolve(setTimer(config.skipDelay, advance));
  });
}

function cancelOnResolve(timerId) {
  return () => clearTimer(timerId);
}

/** Continues past the current line or pause. */
export function advance() {
  const pending = run.pending;
  if (pending && (pending.kind === "say" || pending.kind === "pause")) pending.resolve();
}

/** Waits for `seconds` (or until click when omitted). */
export function pause(seconds) {
  return checkpoint(
    "pause",
    (pending) => {
      if (seconds != null) pending.cleanup = cancelOnResolve(setTimer(seconds * 1000, () => pending.resolve()));
    },
    { rollback: false },
  );
}

function normalizeChoice(choice) {
  if (typeof choice === "string") return { text: choice, value: choice };
  if (Array.isArray(choice)) return { text: choice[0], value: choice[1] ?? choice[0] };
  return { value: choice.text, ...choice };
}

/**
 * Presents choices and resolves with the chosen value.
 *   await menu("Where to?", ["Left", ["Right", "r"], { text: "Secret", value: 3, if: store.key }])
 */
export async function menu(prompt, choices) {
  if (Array.isArray(prompt)) {
    choices = prompt;
    prompt = null;
  }
  const items = choices.map(normalizeChoice).filter((c) => c.if === undefined || c.if);
  const index = await checkpoint(
    "menu",
    (pending) => {
      if (prompt) showScreen("say", { who: null, what: String(prompt), cps: textSpeed() });
      else hideScreen("say");
      showScreen("choice", { items: items.map((c, i) => ({ text: c.text, select: () => pending.resolve(i) })) });
    },
    { record: true },
  );
  hideScreen("choice");
  if (!(index in items)) throw new Error(`saved choice ${index} no longer exists in this menu`);
  return items[index].value;
}

/** Hides the dialogue window until the next line. */
export function windowHide() {
  hideScreen("say");
}

// ---------------------------------------------------------------------------
// Game lifecycle
// ---------------------------------------------------------------------------

function resetPresentation() {
  hideScreen("say");
  hideScreen("choice");
  hideScreen("game_menu");
  invalidate();
}

/** Starts a new game at `start`. */
export function newGame(start = "start") {
  replaceContents(store, structuredCloneJson(storeDefaults));
  scene.bg = null;
  scene.sprites = [];
  scene.music = null;
  replaceScreens([], isGameScreen);
  rngState = (Date.now() ^ 0x9e3779b9) | 0;
  run.root = null;
  run.history = [];
  run.target = -1;
  run.pending = null;
  hideScreen("main_menu");
  resetPresentation();
  startRun(start);
}

/** Leaves the current game and returns to the main menu. */
export function endGame() {
  run.gen++;
  run.root = null;
  run.pending = null;
  run.history = [];
  scene.bg = null;
  scene.sprites = [];
  scene.music = null;
  replaceScreens([], isGameScreen);
  resetPresentation();
  showScreen("main_menu");
}

function restart(root, inputs, target) {
  restore(root.snapshot);
  run.root = root;
  run.inputs = Object.fromEntries(Object.entries(inputs).filter(([k]) => Number(k) < target));
  run.stops = [];
  run.count = 0;
  run.target = target;
  run.pending = null;
  resetPresentation();
  hideScreen("main_menu");
  startRun(root.label, true);
}

/** Steps back to the previous line or choice. Returns false if there is nothing to roll back to. */
export function rollback() {
  if (!run.pending || !run.root) return false;
  for (let i = run.pending.index - 1; i >= 0; i--) {
    if (run.stops[i]) {
      restart(run.root, run.inputs, i);
      return true;
    }
  }
  while (run.history.length) {
    const previous = run.history.pop();
    for (let i = previous.stops.length - 1; i >= 0; i--) {
      if (previous.stops[i]) {
        restart(previous.root, previous.inputs, i);
        return true;
      }
    }
  }
  return false;
}

// ---------------------------------------------------------------------------
// Saving and loading
// ---------------------------------------------------------------------------

export const SAVE_VERSION = 1;

/** True when the game can be saved (the story is waiting for the player). */
export function canSave() {
  return !!(run.pending && run.root);
}

export function saveGame(slot) {
  if (!canSave()) return false;
  storage.write(`save-${slot}`, {
    version: SAVE_VERSION,
    time: Date.now(),
    preview: currentLine.slice(0, 120),
    root: run.root,
    inputs: run.inputs,
    target: run.pending.index,
  });
  return true;
}

export function loadGame(slot) {
  const data = storage.read(`save-${slot}`);
  if (!data || data.version !== SAVE_VERSION) return false;
  if (!labels.has(data.root.label)) {
    reportError(new Error(`save refers to missing label '${data.root.label}'`));
    return false;
  }
  run.history = [];
  restart(data.root, data.inputs, data.target);
  return true;
}

/** Returns save metadata for `slot`, or null. */
export function saveInfo(slot) {
  const data = storage.read(`save-${slot}`);
  return data && { time: data.time, preview: data.preview };
}

export function deleteSave(slot) {
  storage.remove(`save-${slot}`);
}

// ---------------------------------------------------------------------------
// Input
// ---------------------------------------------------------------------------

/** Default key bindings; games may modify this object. */
export const keymap = {
  Enter: "advance",
  " ": "advance",
  Escape: "menu",
  PageUp: "rollback",
  F11: "fullscreen",
};

export const actions = {
  advance(event) {
    if (event?.revealing) command("revealAll");
    else advance();
  },
  rollback() {
    rollback();
  },
  menu() {
    if (!inGame()) return;
    if (isShown("game_menu")) hideScreen("game_menu");
    else showScreen("game_menu", { page: "main" });
  },
  fullscreen() {
    prefs.fullscreen = !prefs.fullscreen;
    savePrefs();
  },
};

on("backgroundClick", (event) => {
  if (event.button === "right") actions.menu(event);
  else if (inGame()) actions.advance(event);
});

on("key", (event) => {
  if (event.key === "Control") {
    skipping = event.down;
    if (skipping && inGame()) advance();
    return;
  }
  if (!event.down) return;
  const action = keymap[event.key];
  if (action === "fullscreen" || (action && inGame())) actions[action]?.(event);
});

on("wheel", (event) => {
  if (!inGame() || isShown("game_menu")) return;
  // Wheel up rolls back, wheel down advances.
  if (event.dy < 0) rollback();
  else if (event.dy > 0) actions.advance(event);
});

on("revealed", () => {
  const pending = run.pending;
  if (!pending || pending.kind !== "say" || !prefs.autoForward || skipping) return;
  const timer = setTimer(prefs.autoDelay * 1000, advance);
  const previous = pending.cleanup;
  pending.cleanup = () => {
    previous?.();
    clearTimer(timer);
  };
});

on("boot", () => {
  Object.assign(prefs, storage.read("prefs") ?? {});
  applyPrefs();
  if (labels.has("splashscreen")) startRun("splashscreen");
  else showScreen("main_menu");
});

on("error", () => {
  run.gen++;
  run.pending = null;
});
