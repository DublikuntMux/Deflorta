// deflorta/scene — images, layered images, positions, transitions, ATL
// transforms, and the serializable scene state (background, sprites, music,
// NVL page) that saves and rollback restore.

import { native, onFlush } from "deflorta/core";
import {
  FILL,
  View,
  exitWith,
  Image,
  invalidate,
  setSceneLayer,
} from "deflorta/ui";

// ---------------------------------------------------------------------------
// Scene state
// ---------------------------------------------------------------------------

/** Serializable scene state; saved and rolled back with the store. */
export const scene = { bg: null, sprites: [], music: null, nvl: [] };

export function resetScene() {
  scene.bg = null;
  scene.sprites = [];
  scene.music = null;
  scene.nvl = [];
}

export function restoreScene(data) {
  resetScene();
  Object.assign(scene, JSON.parse(JSON.stringify(data)));
}

let isReplaying = () => false;

/** Installed by the story runner: sound effects are muted while fast-forwarding. */
export function setReplayCheck(fn) {
  isReplaying = fn;
}

// ---------------------------------------------------------------------------
// Images
// ---------------------------------------------------------------------------

const images = new Map();
const layered = new Map();

/**
 * Declares an image. The first word of the name is its tag: showing
 * "eileen happy" replaces any shown "eileen ..." image.
 * Undeclared names resolve to `images/<name>.png`.
 */
export function image(name, src, options = {}) {
  images.set(name, { src, ...options });
}

/**
 * Declares a layered image composed from attributes:
 *
 *   layeredImage("eileen", [
 *     { src: "images/eileen/base.png" },                               // always shown
 *     { group: "outfit", options: { casual: "…", formal: "…" }, default: "casual" },
 *     { group: "face", options: { happy: "…", sad: "…" }, default: "happy" },
 *     { attribute: "blush", src: "…" },                               // toggled with "blush" / "-blush"
 *   ]);
 *   show("eileen sad blush");
 *
 * The first layer defines the size; all layers must have the same dimensions.
 */
export function layeredImage(tag, layers, options = {}) {
  layered.set(tag, { layers, ...options });
}

const tagOf = (name) => name.split(" ")[0];

/** Resolves new attributes for a layered image, keeping unmentioned groups from `previous`. */
function layeredAttributes(def, requested, previous) {
  const groupOf = new Map();
  for (const layer of def.layers) {
    if (layer.group)
      for (const option of Object.keys(layer.options))
        groupOf.set(option, layer.group);
  }
  let attrs = [...previous];
  for (const attr of requested) {
    if (attr.startsWith("-")) {
      attrs = attrs.filter((a) => a !== attr.slice(1));
      continue;
    }
    const group = groupOf.get(attr);
    if (group) attrs = attrs.filter((a) => groupOf.get(a) !== group);
    if (!attrs.includes(attr)) attrs.push(attr);
  }
  return attrs;
}

/** Image sources of a layered image for the given attributes, bottom to top. */
function layeredSources(def, attrs) {
  const sources = [];
  for (const layer of def.layers) {
    if (layer.group) {
      const chosen =
        Object.keys(layer.options).find((o) => attrs.includes(o)) ??
        layer.default;
      if (chosen) sources.push(layer.options[chosen]);
    } else if (layer.attribute) {
      if (attrs.includes(layer.attribute)) sources.push(layer.src);
    } else {
      sources.push(layer.src);
    }
  }
  return sources;
}

// ---------------------------------------------------------------------------
// Positions, transitions and transforms
// ---------------------------------------------------------------------------

/** Positions: xalign/yalign place the image's anchor point relative to the screen. */
export const left = { xalign: 0.2, yalign: 1 };
export const center = { xalign: 0.5, yalign: 1 };
export const right = { xalign: 0.8, yalign: 1 };
export const truecenter = { xalign: 0.5, yalign: 0.5 };
export const offscreenleft = { xalign: -0.3, yalign: 1 };
export const offscreenright = { xalign: 1.3, yalign: 1 };
/** A custom position; `extra` may set `zoom` and `rotate` (degrees). */
export const at = (xalign, yalign = 1, extra = {}) => ({
  xalign,
  yalign,
  ...extra,
});

/**
 * Transitions describe how elements enter and leave: `in` gives starting
 * values, `out` final values (opacity, x, y, scale, rotate, mask).
 */
export const dissolve = (dur = 0.5) => ({
  dur,
  in: { opacity: 0 },
  out: { opacity: 0 },
});
export const fade = dissolve;
export const moveinleft = (dur = 0.5) => ({
  dur,
  in: { x: -400, opacity: 0 },
  out: { opacity: 0 },
});
export const moveinright = (dur = 0.5) => ({
  dur,
  in: { x: 400, opacity: 0 },
  out: { opacity: 0 },
});
export const moveoutleft = (dur = 0.5) => ({
  dur,
  in: { opacity: 0 },
  out: { x: -400, opacity: 0 },
});
export const moveoutright = (dur = 0.5) => ({
  dur,
  in: { opacity: 0 },
  out: { x: 400, opacity: 0 },
});
export const zoomin = (dur = 0.5) => ({
  dur,
  in: { scale: 0.6, opacity: 0 },
  out: { scale: 0.6, opacity: 0 },
});
/** Slides a shown image to its new position (`show(name, { at, with: move() })`). */
export const move = (dur = 0.5, ease = "easeinout") => ({
  dur,
  move: true,
  ease,
});
/** Reveals the new image in the order of the mask's brightness (dark first). */
export const imageDissolve = (mask, dur = 1, ramp = 0.1) => {
  const m = { mask: { kind: "image", src: mask, ramp } };
  return { dur, ease: "linear", in: m, out: m };
};
const wipe =
  (dir) =>
  (dur = 0.6) => {
    const m = { mask: { kind: "wipe", dir, ramp: 0.08 } };
    return { dur, ease: "linear", in: m, out: m };
  };
export const wipeleft = wipe("left");
export const wiperight = wipe("right");
export const wipeup = wipe("up");
export const wipedown = wipe("down");
export const pixellate = (dur = 1, size = 32) => ({
  dur,
  ease: "linear",
  in: { mask: { kind: "pixellate", size } },
  out: { opacity: 0 },
});

const enterSpec = (t) =>
  t && !t.move ? { dur: t.dur, ease: t.ease, ...t.in } : undefined;
const exitSpec = (t, hold = false) => {
  if (!t || t.move) return undefined;
  return hold
    ? { dur: t.dur, opacity: 1 }
    : { dur: t.dur, ease: t.ease, ...t.out };
};

/**
 * ATL-style animation programs:
 *
 *   const bob = atl().ease(1, { y: -12 }).ease(1, { y: 0 }).repeat();
 *   show("eileen happy", { transform: bob });
 *
 * Properties: x, y (offsets in pixels), opacity, scale, rotate (degrees),
 * crop ([x, y, w, h] fractions). Programs are immutable; each method returns
 * a new program.
 */
export class Atl {
  constructor(steps = []) {
    this.steps = steps;
  }
  #add(step) {
    return new Atl([...this.steps, step]);
  }
  set(props) {
    return this.#add({ set: props });
  }
  tween(dur, props, ease = "linear") {
    return this.#add({ dur, ease, to: props });
  }
  linear(dur, props) {
    return this.tween(dur, props, "linear");
  }
  ease(dur, props) {
    return this.tween(dur, props, "easeinout");
  }
  easeIn(dur, props) {
    return this.tween(dur, props, "easein");
  }
  easeOut(dur, props) {
    return this.tween(dur, props, "easeout");
  }
  bounce(dur, props) {
    return this.tween(dur, props, "bounce");
  }
  pause(seconds) {
    return this.#add({ pause: seconds });
  }
  /** Appends another program. */
  after(program) {
    return new Atl([...this.steps, ...program.steps]);
  }
  /** Repeats the whole program `times` times, or forever. */
  repeat(times = true) {
    return new Atl([{ repeat: times, steps: this.steps }]);
  }
  toJSON() {
    return { steps: this.steps };
  }
}

export const atl = () => new Atl();

/** Runs programs at the same time. */
export const parallel = (...programs) =>
  new Atl([{ parallel: programs.map((p) => p.steps) }]);

/** Ready-made transforms. */
export const shake = (strength = 12, dur = 0.4) =>
  atl()
    .linear(dur / 8, { x: strength })
    .linear(dur / 4, { x: -strength })
    .linear(dur / 4, { x: strength / 2 })
    .linear(dur / 4, { x: -strength / 2 })
    .linear(dur / 8, { x: 0 });
export const bob = (height = 10, period = 2) =>
  atl()
    .ease(period / 2, { y: -height })
    .ease(period / 2, { y: 0 })
    .repeat();

// ---------------------------------------------------------------------------
// Scene statements
// ---------------------------------------------------------------------------

const pendingEnters = new Map();
const pendingMoves = new Map();
let musicFade = { fadeIn: 0, fadeOut: 0 };
let sentMusic = "null";

const spriteKey = (s) => `sprite:${s.tag}:${s.name}`;

/** Clears the screen and optionally shows a background. */
export function setScene(name = null, options = {}) {
  const t = options.with;
  if (scene.bg) exitWith(`bg:${scene.bg}`, exitSpec(t, true));
  for (const s of scene.sprites) exitWith(spriteKey(s), exitSpec(t));
  scene.sprites = [];
  scene.bg = name;
  if (name) pendingEnters.set(`bg:${name}`, enterSpec(t));
  invalidate();
}

/**
 * Shows an image, replacing any image with the same tag.
 * options: at (position), with (transition), zorder, transform (Atl or null to clear).
 */
export function show(name, options = {}) {
  const [tag, ...requested] = name.split(" ").filter(Boolean);
  const t = options.with;
  const index = scene.sprites.findIndex((s) => s.tag === tag);
  const previous = scene.sprites[index];
  let resolvedName = name;
  let attrs;
  const def = layered.get(tag);
  if (def) {
    attrs = layeredAttributes(def, requested, previous?.attrs ?? []);
    resolvedName = [tag, ...attrs].join(" ");
  }
  const transform =
    options.transform === undefined
      ? previous?.transform
      : options.transform && JSON.parse(JSON.stringify(options.transform));
  const entry = {
    tag,
    name: resolvedName,
    attrs,
    at: options.at ?? previous?.at ?? center,
    zorder: options.zorder ?? previous?.zorder ?? 0,
    transform,
  };
  const key = spriteKey(entry);
  if (previous && previous.name !== resolvedName)
    exitWith(spriteKey(previous), exitSpec(t));
  if (!previous || previous.name !== resolvedName)
    pendingEnters.set(key, enterSpec(t));
  else if (t?.move) pendingMoves.set(key, { dur: t.dur, ease: t.ease });
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
  exitWith(spriteKey(previous), exitSpec(options.with));
  scene.sprites = scene.sprites.filter((s) => s.tag !== tag);
  invalidate();
}

/** Image files the current scene will need (for preloading). */
export function imageSources(name) {
  const [tag, ...attrs] = name.split(" ");
  const def = layered.get(tag);
  if (def) return layeredSources(def, layeredAttributes(def, attrs, []));
  return [(images.get(name) ?? { src: `images/${name}.png` }).src];
}

/** Starts decoding images before they are shown (e.g. at the start of a chapter). */
export function preload(...names) {
  native.ui.preload(names.flatMap(imageSources));
}

function renderSprite(s) {
  const key = spriteKey(s);
  const at = s.at;
  const props = {
    key,
    anchor: [at.xalign, at.yalign],
    enter: pendingEnters.get(key),
    move: pendingMoves.get(key),
    transform: s.transform ?? undefined,
  };
  const style = {
    position: "absolute",
    left: `${at.xalign * 100}%`,
    top: `${at.yalign * 100}%`,
    rotate: at.rotate,
  };
  const def = layered.get(s.tag);
  if (def) {
    const [first, ...rest] = layeredSources(def, s.attrs ?? []);
    const layerStyle = {
      position: "absolute",
      left: 0,
      top: 0,
      width: "100%",
      height: "100%",
    };
    return (
      <View
        {...props}
        style={{ ...style, scale: (at.zoom ?? 1) * (def.zoom ?? 1) }}
      >
        {first && <Image src={first} />}
        {rest.map((src, i) => (
          <Image key={`layer-${i}`} src={src} style={layerStyle} />
        ))}
      </View>
    );
  }
  const spec = images.get(s.name) ?? { src: `images/${s.name}.png` };
  return (
    <Image
      src={spec.src}
      {...props}
      style={{ ...style, scale: (at.zoom ?? 1) * (spec.zoom ?? 1) }}
    />
  );
}

function renderScene() {
  const children = [];
  if (scene.bg) {
    const key = `bg:${scene.bg}`;
    const spec = images.get(scene.bg) ?? { src: `images/${scene.bg}.png` };
    children.push(
      <Image
        src={spec.src}
        key={key}
        fit="cover"
        style={FILL}
        enter={pendingEnters.get(key)}
      />,
    );
  }
  for (const s of scene.sprites) children.push(renderSprite(s));
  return (
    <View key="scene" style={FILL}>
      {children}
    </View>
  );
}

setSceneLayer(renderScene);

onFlush(() => {
  pendingEnters.clear();
  pendingMoves.clear();
  const music = JSON.stringify(scene.music);
  if (music === sentMusic) return;
  sentMusic = music;
  native.audio.music(scene.music, musicFade);
  musicFade = { fadeIn: 0, fadeOut: 0 };
});

// ---------------------------------------------------------------------------
// Audio
// ---------------------------------------------------------------------------

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
    if (!isReplaying()) native.audio.sound(file, volume);
  },
};
