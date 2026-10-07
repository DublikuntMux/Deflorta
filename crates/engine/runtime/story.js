import {
  clearTimer,
  config,
  native,
  on,
  reportError,
  setTimer,
  storage,
} from "deflorta/core";
import {
  hideScreen,
  invalidate,
  isShown,
  markInstant,
  replaceScreens,
  screenProps,
  setUiHidden,
  showScreen,
  shownScreens,
} from "deflorta/ui";
import { _, parseMarkup, plainText, useLanguage } from "deflorta/text";
import {
  resetScene,
  restoreScene,
  scene,
  setReplayCheck,
  setScene,
} from "deflorta/scene";

export const store = {};
const storeDefaults = {};

export function defaults(values) {
  Object.assign(storeDefaults, clone(values));
  for (const [k, v] of Object.entries(values))
    if (!(k in store)) store[k] = clone(v);
}

function clone(value) {
  return value === undefined ? undefined : JSON.parse(JSON.stringify(value));
}

function replaceContents(target, source) {
  for (const k of Object.keys(target)) delete target[k];
  Object.assign(target, source);
}

export const persistent = {};
let persistentJson = "{}";

export function savePersistent() {
  const json = JSON.stringify(persistent);
  if (json === persistentJson) return;
  persistentJson = json;
  storage.write("persistent", persistent);
}

export const prefs = {
  textSpeed: null,
  autoForward: false,
  autoDelay: 1.5,
  musicVolume: 0.8,
  soundVolume: 0.8,
  voiceVolume: 1,
  voiceSustain: false,
  selfVoicing: false,
  skipUnseen: false,
  fullscreen: false,
  language: null,
};

export function savePrefs() {
  storage.write("prefs", prefs);
  applyPrefs();
}

function applyPrefs() {
  native.audio.volume("music", prefs.musicVolume);
  native.audio.volume("sound", prefs.soundVolume);
  native.audio.volume("voice", prefs.voiceVolume);
  native.app.fullscreen(prefs.fullscreen);
  native.app.selfVoicing(prefs.selfVoicing);
  useLanguage(prefs.language);
  refreshDialogue();
  syncAutomaticAdvance();
  invalidate();
}

export function setLanguage(language) {
  console.info(`Language: ${language ?? "source"}`);
  prefs.language = language;
  savePrefs();
}

export const INSTANT_SPEED = 200;

export function textSpeed() {
  const speed = prefs.textSpeed ?? config.textSpeed;
  return speed >= INSTANT_SPEED ? 0 : speed;
}

// Deterministic random numbers (mulberry32), part of the saved state.
let rngState = 1 >>> 0;

export function random() {
  rngState = (rngState + 0x6d2b79f5) >>> 0;
  let t = rngState;
  t = Math.imul(t ^ (t >>> 15), t | 1);
  t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
  return ((t ^ (t >>> 14)) >>> 0) / 0x100000000;
}

export function randInt(min, max) {
  return min + Math.floor(random() * (max - min + 1));
}

let seen = {};
let seenDirty = 0;

function lineKey(label, text) {
  // FNV-1a over label and text.
  let h = 0x811c9dc5;
  const s = `${label}\u0000${text}`;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return (h >>> 0).toString(36);
}

function markSeen(key) {
  if (seen[key]) return;
  seen[key] = 1;
  if (++seenDirty >= 10) flushSeen();
}

function flushSeen() {
  if (!seenDirty) return;
  seenDirty = 0;
  storage.write("seen", seen);
}

const labels = new Map();

export function label(name, fn) {
  labels.set(name, fn);
}

class Jump {
  constructor(target) {
    this.target = target;
  }
}

class ReplayMismatch extends Error {}

export function jump(name) {
  throw new Jump(name);
}

export async function call(name, ...args) {
  const fn = labels.get(name);
  if (!fn) throw new Error(`unknown label '${name}'`);
  return fn(...args);
}

const MAX_ROOTS = 64;
const MAX_LINES = 250;

const run = {
  gen: 0,
  nextRootId: 1,
  root: null, // { id, label, snapshot } at the start of the current label
  count: 0, // checkpoints reached since the root
  inputs: {}, // checkpoint index -> recorded input
  stops: [], // checkpoint index -> true if rollback may stop there
  kinds: [], // checkpoint index -> kind, to detect saves from other script versions
  expected: null, // kinds recorded in the save being loaded
  target: -1, // when >= 0, fast-forward until this checkpoint
  recovering: false,
  pending: null, // the checkpoint waiting for the player
  roots: [], // earlier roots, for rolling back across jumps
  autosaveDue: false,
};

const replaying = () => run.target >= 0;
setReplayCheck(replaying);

export const history = [];

// Screens managed by the runtime; every other shown screen is game state and
// is saved, restored and rolled back with the scene.
export const SYSTEM_SCREENS = new Set([
  "say",
  "nvl",
  "choice",
  "input",
  "movie",
  "quick_menu",
  "history",
  "game_menu",
  "main_menu",
  "confirm",
  "notify",
  "tooltip",
  "error",
]);
const isGameScreen = (name) => !SYSTEM_SCREENS.has(name);

function snapshot() {
  return JSON.stringify({
    store,
    scene,
    screens: shownScreens(isGameScreen),
    rng: rngState,
  });
}

function restore(snap) {
  const data = JSON.parse(snap);
  replaceContents(store, data.store);
  restoreScene(data.scene);
  replaceScreens(data.screens ?? [], isGameScreen);
  rngState = data.rng;
}

function beginRoot(name) {
  if (replaying())
    throw new ReplayMismatch(`jumped to '${name}' while restoring`);
  if (run.root) {
    run.roots.push({ root: run.root, inputs: run.inputs, stops: run.stops });
    if (run.roots.length > MAX_ROOTS) run.roots.shift();
  }
  run.root = { id: run.nextRootId++, label: name, snapshot: snapshot() };
  console.debug(`Entering label '${name}'`);
  run.count = 0;
  run.inputs = {};
  run.stops = [];
  run.kinds = [];
  run.autosaveDue = true;
}

async function runStory(start, resume = false) {
  const gen = ++run.gen;
  let name = start;
  try {
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
    if (run.gen !== gen) return;
    if (replaying())
      throw new ReplayMismatch("the scene ended while restoring");
    endGame();
  } catch (e) {
    if (run.gen !== gen) return;
    if (replaying() && !run.recovering) {
      recover(e);
      return;
    }
    throw e;
  }
}

function recover(error) {
  console.warn(
    `cannot restore position (${error?.message ?? error}); restarting the scene`,
  );
  emitNotice(
    _("The game was updated since this save. Restarting the current scene."),
  );
  const root = run.root;
  history.splice(0, history.length, ...history.filter((h) => h.root < root.id));
  run.recovering = true;
  restart(root, {}, -1, null);
}

let noticeFn = () => {};

export function setNoticeHandler(fn) {
  noticeFn = fn;
}

function emitNotice(message) {
  noticeFn(message);
}

function startRun(start, resume) {
  runStory(start, resume).catch((e) => reportError(e));
}

export function inGame() {
  return run.root !== null;
}

export function checkpoint(
  kind,
  present,
  { record = false, rollback = true } = {},
) {
  const index = run.count++;
  run.stops[index] = rollback;
  run.kinds[index] = kind;
  if (
    run.expected &&
    run.expected[index] &&
    run.expected[index] !== kind &&
    index <= run.target
  ) {
    return Promise.reject(
      new ReplayMismatch(
        `expected ${run.expected[index]} at ${index}, found ${kind}`,
      ),
    );
  }
  if (index < run.target)
    return Promise.resolve(record ? run.inputs[index] : undefined);
  if (index === run.target) {
    run.target = -1;
    run.expected = null;
    markInstant();
  }
  run.recovering = false;
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
    if (run.autosaveDue && config.autosave !== false) {
      run.autosaveDue = false;
      autosave();
    }
  });
}

function addCleanup(pending, fn) {
  const previous = pending.cleanup;
  pending.cleanup = () => {
    previous?.();
    fn();
  };
}

function cancelCheckpoint() {
  const pending = run.pending;
  run.pending = null;
  pending?.cleanup?.();
}

let skipHeld = false;
let skipToggled = false;
let currentLine = "";
let nextVoice = null;
let revealSeq = 0;

function storyBlocked() {
  return ["game_menu", "history", "confirm"].some(isShown);
}

function refreshDialogue() {
  const pending = run.pending;
  if (pending?.kind !== "say" || typeof pending.markup !== "string") return;
  const translated = _(pending.markup);
  const previousNoWait = pending.noWait;
  pending.noWait = parseMarkup(translated).noWait;
  currentLine = pending.speaker
    ? `${_(pending.speaker.name)}: ${plainText(translated)}`
    : plainText(translated);
  if (pending.revealed && previousNoWait !== pending.noWait) {
    if (pending.noWait) {
      scheduleAutomaticAdvance(pending, 0, "noWait");
    } else if (pending.automatic?.mode === "noWait") {
      if (pending.automatic.timer != null) clearTimer(pending.automatic.timer);
      pending.automatic = null;
      if (prefs.autoForward)
        scheduleAutomaticAdvance(pending, prefs.autoDelay * 1000, "auto");
    }
  }
}

function automaticEnabled(pending) {
  if (pending.automatic.mode === "skip") return isSkipping();
  if (pending.automatic.mode === "noWait") return pending.noWait;
  return prefs.autoForward;
}

// Automatic progression belongs to one checkpoint. Keep the remaining delay
// while a blocking screen is open, including menus opened by custom screens.
function syncAutomaticAdvance() {
  const pending = run.pending;
  const automatic = pending?.automatic;
  if (!automatic) return;
  if (!automaticEnabled(pending) || storyBlocked()) {
    if (automatic.timer != null) {
      automatic.remaining = Math.max(0, automatic.due - Date.now());
      clearTimer(automatic.timer);
      automatic.timer = null;
    }
    return;
  }
  if (automatic.timer != null) return;
  automatic.due = Date.now() + automatic.remaining;
  automatic.timer = setTimer(automatic.remaining, () => {
    automatic.timer = null;
    automatic.remaining = 0;
    if (run.pending !== pending || storyBlocked()) return;
    if (automaticEnabled(pending)) pending.resolve();
  });
}

function scheduleAutomaticAdvance(pending, delay, mode) {
  if (pending.automatic?.timer != null) clearTimer(pending.automatic.timer);
  pending.automatic = { remaining: delay, timer: null, due: 0, mode };
  if (!pending.automaticCleanup) {
    pending.automaticCleanup = true;
    addCleanup(pending, () => {
      if (pending.automatic?.timer != null) clearTimer(pending.automatic.timer);
      pending.automatic = null;
    });
  }
  syncAutomaticAdvance();
}

on("screensChanged", syncAutomaticAdvance);

export const isSkipping = () => skipHeld || skipToggled;

export function toggleSkip(on = !skipToggled) {
  skipToggled = on;
  if (on && !storyBlocked()) advance();
  syncAutomaticAdvance();
  invalidate();
}

export function character(name, options = {}) {
  const who = { name, color: "#ffffff", ...options };
  const speak = (first, ...rest) =>
    Array.isArray(first) && first.raw
      ? say(who, String.raw(first, ...rest))
      : say(who, first, rest[0]);
  speak.who = who;
  return speak;
}

export const nvlNarrator = character(null, { nvl: true });

export function voice(file) {
  nextVoice = file;
}

export function nvlClear() {
  scene.nvl = [];
  invalidate();
}

export function say(who, what, options = {}) {
  if (what === undefined) {
    what = who;
    who = null;
  }
  if (typeof who === "string") who = { name: who };
  const markup = String(what);
  const speaker =
    who?.name != null ? { name: who.name, color: who.color } : null;
  const voiceFile = options.voice ?? nextVoice;
  nextVoice = null;
  const nvl = !!who?.nvl;
  const key = lineKey(run.root?.label ?? "", markup);
  if (nvl) scene.nvl = [...scene.nvl, { who: speaker, what: markup }];

  history.push({
    who: speaker,
    what: markup,
    voice: voiceFile,
    root: run.root?.id,
    index: run.count,
  });
  if (history.length > MAX_LINES) history.shift();

  return checkpoint("say", (pending) => {
    pending.markup = markup;
    pending.speaker = speaker;
    refreshDialogue();
    const revealKey = ++revealSeq;
    if (nvl) {
      hideScreen("say");
      showScreen("nvl", { lines: scene.nvl, cps: textSpeed(), revealKey, ...options });
    } else {
      hideScreen("nvl");
      showScreen("say", {
        who,
        what: markup,
        cps: textSpeed(),
        revealKey,
        ...options,
      });
    }
    if (voiceFile) {
      native.audio.voice(voiceFile);
      if (!prefs.voiceSustain)
        addCleanup(pending, () => native.audio.voice(null));
    }
    const wasSeen = !!seen[key];
    markSeen(key);
    if (isSkipping()) {
      if (wasSeen || prefs.skipUnseen) {
        scheduleAutomaticAdvance(pending, config.skipDelay, "skip");
      } else {
        skipToggled = false;
      }
    }
  });
}

export function advance() {
  const pending = run.pending;
  if (
    pending &&
    (pending.kind === "say" ||
      pending.kind === "pause" ||
      pending.kind === "movie")
  ) {
    if (pending.kind === "movie" && !pending.skippable) return;
    pending.resolve();
  }
}

export function pause(seconds) {
  return checkpoint(
    "pause",
    (pending) => {
      if (seconds != null) {
        const timer = setTimer(seconds * 1000, () => pending.resolve());
        addCleanup(pending, () => clearTimer(timer));
      }
    },
    { rollback: false },
  );
}

function normalizeChoice(choice) {
  if (typeof choice === "string") return { text: choice, value: choice };
  if (Array.isArray(choice))
    return { text: choice[0], value: choice[1] ?? choice[0] };
  return { value: choice.text, ...choice };
}

export async function menu(prompt, choices) {
  if (Array.isArray(prompt)) {
    choices = prompt;
    prompt = null;
  }
  const items = choices
    .map(normalizeChoice)
    .filter((c) => c.if === undefined || c.if);
  if (prompt)
    history.push({
      who: null,
      what: String(prompt),
      root: run.root?.id,
      index: run.count,
    });
  const index = await checkpoint(
    "menu",
    (pending) => {
      if (scene.nvl.length) showScreen("nvl", { lines: scene.nvl, cps: 0 });
      else if (prompt)
        showScreen("say", {
          who: null,
          what: String(prompt),
          cps: textSpeed(),
          revealKey: ++revealSeq,
        });
      else hideScreen("say");
      skipToggled = false;
      showScreen("choice", {
        items: items.map((c, i) => ({
          text: c.text,
          select: () => pending.resolve(i),
        })),
      });
    },
    { record: true },
  );
  hideScreen("choice");
  if (!(index in items))
    throw new ReplayMismatch(
      `saved choice ${index} no longer exists in this menu`,
    );
  history.push({
    who: null,
    what: `» ${items[index].text}`,
    choice: true,
    root: run.root?.id,
    index: run.count - 1,
  });
  return items[index].value;
}

export function prompt(
  question,
  { default: initial = "", maxLength = 32, allowEmpty = false } = {},
) {
  return checkpoint(
    "input",
    (pending) => {
      showScreen("input", {
        question,
        value: initial,
        maxLength,
        submit(value) {
          const answer = String(value ?? "").trim();
          if (!answer && !allowEmpty) return;
          pending.resolve(answer);
        },
      });
      addCleanup(pending, () => hideScreen("input"));
    },
    { record: true },
  );
}

export function playMovie(src, { skippable = true } = {}) {
  return checkpoint(
    "movie",
    (pending) => {
      pending.skippable = skippable;
      showScreen("movie", { src, end: () => pending.resolve() });
      addCleanup(pending, () => hideScreen("movie"));
    },
    { rollback: false },
  );
}

export function sceneStatement(name = null, options = {}) {
  setScene(name, options);
  hideScreen("say");
  hideScreen("nvl");
}

export function windowHide() {
  hideScreen("say");
  hideScreen("nvl");
}

function resetPresentation() {
  for (const name of [
    "say",
    "nvl",
    "choice",
    "input",
    "movie",
    "history",
    "game_menu",
    "confirm",
  ])
    hideScreen(name);
  setUiHidden(false);
  invalidate();
}

export function newGame(start = "start") {
  console.info(`New game at label '${start}'`);
  cancelCheckpoint();
  replaceContents(store, clone(storeDefaults));
  resetScene();
  replaceScreens([], isGameScreen);
  rngState = (Date.now() ^ 0x9e3779b9) | 0;
  history.length = 0;
  Object.assign(run, {
    root: null,
    roots: [],
    target: -1,
    expected: null,
    pending: null,
    recovering: false,
  });
  skipToggled = false;
  hideScreen("main_menu");
  resetPresentation();
  showScreen("quick_menu");
  startRun(start);
}

export function endGame() {
  console.info("Returning to the main menu");
  run.gen++;
  cancelCheckpoint();
  Object.assign(run, {
    root: null,
    pending: null,
    roots: [],
    target: -1,
    expected: null,
  });
  resetScene();
  history.length = 0;
  replaceScreens([], isGameScreen);
  resetPresentation();
  hideScreen("quick_menu");
  showScreen("main_menu");
  savePersistent();
  flushSeen();
}

function restart(root, inputs, target, expected) {
  console.debug(`Restoring label '${root.label}' at checkpoint ${target}`);
  cancelCheckpoint();
  restore(root.snapshot);
  run.root = root;
  run.nextRootId = Math.max(run.nextRootId, root.id + 1);
  run.inputs = Object.fromEntries(
    Object.entries(inputs).filter(([k]) => Number(k) < target),
  );
  run.stops = [];
  run.kinds = [];
  run.expected = expected;
  run.count = 0;
  run.target = target;
  run.pending = null;
  run.autosaveDue = false;
  skipToggled = false;
  const kept = history.filter((h) => h.root < root.id);
  history.splice(0, history.length, ...kept);
  resetPresentation();
  hideScreen("main_menu");
  showScreen("quick_menu");
  markInstant();
  startRun(root.label, true);
}

export function rollback() {
  if (!run.pending || !run.root) return false;
  console.debug("Rolling back");
  for (let i = run.pending.index - 1; i >= 0; i--) {
    if (run.stops[i]) {
      restart(run.root, run.inputs, i, null);
      return true;
    }
  }
  while (run.roots.length) {
    const previous = run.roots.pop();
    for (let i = previous.stops.length - 1; i >= 0; i--) {
      if (previous.stops[i]) {
        restart(previous.root, previous.inputs, i, null);
        return true;
      }
    }
  }
  return false;
}

export function rollbackTo(entry) {
  if (!run.root || entry.root == null) return false;
  console.debug("Rolling back to a history entry");
  if (entry.root === run.root.id) {
    if (run.pending && entry.index >= run.pending.index) return false;
    restart(run.root, run.inputs, entry.index, null);
    return true;
  }
  const i = run.roots.findIndex((r) => r.root.id === entry.root);
  if (i < 0) return false;
  const [target] = run.roots.splice(i);
  restart(target.root, target.inputs, entry.index, null);
  return true;
}

export const SAVE_VERSION = 2;

export function canSave() {
  return !!(run.pending && run.root);
}

export function saveGame(slot, { thumbnail = true } = {}) {
  if (!canSave()) return false;
  const target = run.pending.index;
  storage.write(`save-${slot}`, {
    version: SAVE_VERSION,
    gameVersion: config.version ?? null,
    time: Date.now(),
    preview: currentLine.slice(0, 120),
    root: run.root,
    inputs: run.inputs,
    target,
    kinds: run.kinds.slice(0, target + 1),
    history: history.filter((h) => h.root < run.root.id).slice(-100),
  });
  if (thumbnail) native.ui.saveThumbnail(`thumb-${slot}`);
  savePersistent();
  flushSeen();
  console.info(
    `Saved slot '${slot}' (label '${run.root.label}', checkpoint ${target})`,
  );
  return true;
}

export function loadGame(slot) {
  const data = storage.read(`save-${slot}`);
  if (!data || data.version !== SAVE_VERSION) {
    console.warn(
      `Cannot load slot '${slot}': ${data ? `save format ${data.version}` : "no save"}`,
    );
    return false;
  }
  if (!labels.has(data.root.label)) {
    reportError(new Error(`save refers to missing label '${data.root.label}'`));
    return false;
  }
  run.gen++;
  run.roots = [];
  run.recovering = false;
  history.splice(0, history.length, ...(data.history ?? []));
  console.info(
    `Loading slot '${slot}' (label '${data.root.label}', game version ${data.gameVersion ?? "unset"})`,
  );
  restart(data.root, data.inputs, data.target, data.kinds ?? null);
  return true;
}

export function saveInfo(slot) {
  const data = storage.read(`save-${slot}`);
  return (
    data && {
      time: data.time,
      preview: data.preview,
      thumbnail: `user:thumb-${slot}.png?${data.time}`,
    }
  );
}

export function deleteSave(slot) {
  storage.remove(`save-${slot}`);
  native.ui.deleteThumbnail(`thumb-${slot}`);
  console.info(`Deleted slot '${slot}'`);
}

export function quickSave() {
  native.ui.captureThumbnail(false);
  return saveGame("quick");
}

export function quickLoad() {
  return loadGame("quick");
}

export const AUTOSAVE_SLOTS = 6;

export function autosave({ now = false } = {}) {
  if (!canSave()) return false;
  const index = ((persistent._autosave ?? 0) % AUTOSAVE_SLOTS) + 1;
  persistent._autosave = index;
  native.ui.captureThumbnail(!now);
  return saveGame(`auto-${index}`);
}

export const keymap = {
  Enter: "advance",
  " ": "advance",
  Escape: "menu",
  PageUp: "rollback",
  Tab: "skip",
  h: "history",
  F5: "quickSave",
  F6: "selfVoicing",
  F9: "quickLoad",
  F11: "fullscreen",
};

let gameMenuNotice = () => {};

export function setQuickNotice(fn) {
  gameMenuNotice = fn;
}

export const actions = {
  selfVoicing(event) {
    if (event?.repeat) return;
    prefs.selfVoicing = !prefs.selfVoicing;
    savePrefs();
  },
  advance(event) {
    if (event?.revealing) native.ui.revealSkip();
    else advance();
  },
  rollback() {
    rollback();
  },
  menu() {
    if (!inGame()) return;
    if (isShown("game_menu")) {
      hideScreen("game_menu");
      return;
    }
    // Capture the game screen (without the menu) for save thumbnails.
    native.ui.captureThumbnail(false);
    showScreen("game_menu", { page: "main" });
  },
  history() {
    if (!inGame()) return;
    if (isShown("history")) hideScreen("history");
    else showScreen("history");
  },
  skip() {
    toggleSkip();
  },
  auto() {
    prefs.autoForward = !prefs.autoForward;
    savePrefs();
    if (prefs.autoForward && run.pending?.kind === "say" && !storyBlocked()) advance();
  },
  hideUi() {
    setUiHidden(true);
  },
  quickSave() {
    if (quickSave()) gameMenuNotice(_("Quick saved"));
  },
  quickLoad() {
    if (!quickLoad()) gameMenuNotice(_("No quick save"));
  },
  fullscreen() {
    prefs.fullscreen = !prefs.fullscreen;
    savePrefs();
  },
};

const GLOBAL_ACTIONS = new Set(["fullscreen", "selfVoicing"]);

on("backgroundClick", (event) => {
  if (event.button === "right") actions.menu(event);
  else if (event.button === "middle") inGame() && actions.hideUi();
  else if (inGame()) actions.advance(event);
});

on("key", (event) => {
  if (event.key === "Control") {
    skipHeld = event.down;
    if (skipHeld && inGame() && !storyBlocked()) advance();
    syncAutomaticAdvance();
    invalidate();
    return;
  }
  if (!event.down) return;
  const action = keymap[event.key];
  if (action && (GLOBAL_ACTIONS.has(action) || inGame()))
    actions[action]?.(event);
});

on("wheel", (event) => {
  if (!inGame() || storyBlocked()) return;
  if (event.dy < 0) rollback();
  else if (event.dy > 0) actions.advance(event);
});

on("revealed", () => {
  const pending = run.pending;
  if (!pending || pending.kind !== "say" || isSkipping()) return;
  pending.revealed = true;
  if (pending.noWait) {
    scheduleAutomaticAdvance(pending, 0, "noWait");
    return;
  }
  if (!prefs.autoForward) return;
  scheduleAutomaticAdvance(pending, prefs.autoDelay * 1000, "auto");
});

on("boot", () => {
  Object.assign(prefs, storage.read("prefs") ?? {});
  Object.assign(persistent, storage.read("persistent") ?? {});
  persistentJson = JSON.stringify(persistent);
  seen = storage.read("seen") ?? {};
  console.info(
    `Story runtime ready: ${labels.size} labels, ${Object.keys(seen).length} lines seen`,
  );
  applyPrefs();
  if (labels.has("splashscreen")) startRun("splashscreen");
  else showScreen("main_menu");
});

on("quit", () => {
  console.info("Saving persistent data before quitting");
  if (canSave() && config.autosave !== false) autosave({ now: true });
  savePersistent();
  flushSeen();
});

on("error", () => {
  run.gen++;
  cancelCheckpoint();
});

export function updatePromptValue(value) {
  const props = screenProps("input");
  if (props) props.value = value;
}
