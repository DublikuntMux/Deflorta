// deflorta/ui — declarative UI elements and the screen stack.
//
// Screens are functions returning an element tree. Whenever state changes,
// call invalidate(); the whole tree is re-rendered and sent to the engine,
// which lays it out (flexbox), animates and draws it.

import { addFrameSource, emit, on } from "deflorta/core";

// ---------------------------------------------------------------------------
// Elements
// ---------------------------------------------------------------------------

function flatten(children, out = []) {
  for (const child of children) {
    if (Array.isArray(child)) flatten(child, out);
    else if (child != null && child !== false && child !== true) out.push(child);
  }
  return out;
}

/** A flexbox container. `box({ style, onClick, hover, key }, ...children)` */
export function box(props = {}, ...children) {
  return { t: "box", ...props, children: flatten(children) };
}

/** A text run. Style props: color, fontSize, fontFamily, fontWeight, italic, lineHeight, textAlign, textShadow. */
export function text(content, props = {}) {
  return { t: "text", text: String(content ?? ""), ...props };
}

/** An image from the game directory. `fit`: "cover" | "contain" | "fill". */
export function img(src, props = {}) {
  return { t: "image", src, ...props };
}

export const FILL = { position: "absolute", left: 0, top: 0, right: 0, bottom: 0 };

/** A clickable box with a text label and hover feedback. */
export function button(label, onClick, props = {}) {
  const { style, hover, textStyle, disabled, ...rest } = props;
  return box(
    {
      style: {
        padding: [10, 24],
        radius: 8,
        background: "#ffffff14",
        justifyContent: "center",
        alignItems: "center",
        color: disabled ? "#ffffff55" : "#f2f2f2",
        ...style,
      },
      hover: disabled ? undefined : { background: "#ffffff30", color: "#ffffff", ...hover },
      onClick: disabled ? undefined : onClick,
      ...rest,
    },
    text(label, { style: textStyle }),
  );
}

// ---------------------------------------------------------------------------
// Screens
// ---------------------------------------------------------------------------

const screens = new Map();
let shown = [];
let showSeq = 0;
let dirty = true;
let instant = false;
let sceneLayer = () => null;
const exits = {};

/**
 * Defines (or replaces) a screen.
 * options.z      stacking order, higher is on top
 * options.modal  block clicks and keys from reaching lower screens
 * options.keys   { [key]: (event) => void } handled while the screen is shown
 */
export function screen(name, render, options = {}) {
  screens.set(name, { render, z: options.z ?? 0, modal: !!options.modal, keys: options.keys ?? {} });
  invalidate();
}

export function showScreen(name, props = {}) {
  shown = shown.filter((s) => s.name !== name);
  shown.push({ name, props, seq: ++showSeq });
  shown.sort((a, b) => (screens.get(a.name)?.z ?? 0) - (screens.get(b.name)?.z ?? 0) || a.seq - b.seq);
  invalidate();
}

export function hideScreen(name) {
  const before = shown.length;
  shown = shown.filter((s) => s.name !== name);
  if (shown.length !== before) invalidate();
}

export function isShown(name) {
  return shown.some((s) => s.name === name);
}

/** Names and props of shown screens accepted by `filter`, bottom to top. */
export function shownScreens(filter = () => true) {
  return shown.filter((s) => filter(s.name)).map(({ name, props }) => ({ name, props }));
}

/** Replaces every shown screen accepted by `filter` with `list`. */
export function replaceScreens(list, filter = () => true) {
  shown = shown.filter((s) => !filter(s.name));
  for (const { name, props } of list) showScreen(name, props);
  invalidate();
}

export function screenProps(name) {
  return shown.find((s) => s.name === name)?.props;
}

/** Marks the UI as changed; it is re-rendered before the next frame. */
export function invalidate() {
  dirty = true;
}

/** The next commit skips enter/exit animations (used after loading and rollback). */
export function markInstant() {
  instant = true;
  dirty = true;
}

/** Plays `spec` as the exit animation of the element with `key` if it disappears in the next commit. */
export function exitWith(key, spec) {
  exits[key] = spec;
}

/** Installs the function rendering the game scene underneath all screens. */
export function setSceneLayer(fn) {
  sceneLayer = fn;
  invalidate();
}

// ---------------------------------------------------------------------------
// Rendering and event routing
// ---------------------------------------------------------------------------

let handlers = [];

function renderRoot() {
  const children = [sceneLayer()];
  for (const entry of shown) {
    const def = screens.get(entry.name);
    if (!def) continue;
    const content = def.render(entry.props);
    if (def.modal) {
      children.push(box({ key: `screen:${entry.name}`, style: FILL, onClick: () => {} }, content));
    } else if (content) {
      children.push({ ...content, key: content.key ?? `screen:${entry.name}` });
    }
  }
  return box({ key: "root", style: FILL, onClick: (event) => emit("backgroundClick", event) }, children);
}

// Copies the tree, replacing click handlers with indices into `handlers`.
function serialize(node) {
  const out = {};
  for (const key in node) {
    const value = node[key];
    if (key === "children") {
      out.children = value.map(serialize);
    } else if (key === "onClick") {
      if (typeof value === "function") out.onClick = handlers.push(value) - 1;
    } else if (typeof value !== "function" && value !== undefined) {
      out[key] = value;
    }
  }
  return out;
}

addFrameSource(() => {
  if (!dirty) return;
  dirty = false;
  handlers = [];
  const out = { tree: serialize(renderRoot()) };
  if (instant) out.instant = true;
  if (Object.keys(exits).length) {
    out.exits = { ...exits };
    for (const k of Object.keys(exits)) delete exits[k];
  }
  instant = false;
  return out;
});

on("click", (event) => {
  if (event.h != null) handlers[event.h]?.(event);
});

on("key", (event) => {
  if (!event.down) return;
  for (let i = shown.length - 1; i >= 0; i--) {
    const def = screens.get(shown[i].name);
    if (!def) continue;
    const handler = def.keys[event.key];
    if (handler) {
      handler(event);
      return true;
    }
    if (def.modal) return true;
  }
});
