// deflorta/ui — declarative UI elements, widgets and the screen stack.
//
// Screens are functions returning an element tree. Whenever state changes,
// call invalidate(); at the end of the turn the whole tree is re-rendered and
// committed to the engine, which reads it in place, lays it out
// (flexbox/grid), animates and draws it. Handler functions stay in JS; the
// engine hands them back in click and handler events. Keyboard and
// gamepad focus moves between elements with handlers; focused elements use
// their `hover` style.

import { emit, native, on, onFlush } from "deflorta/core";
import { parseMarkup } from "deflorta/text";

// ---------------------------------------------------------------------------
// Theme: shared look of all default screens
// ---------------------------------------------------------------------------

export const theme = {
  font: null,
  accent: "#e8a8c8",
  text: "#f3f1f5",
  mutedText: "#ffffffaa",
  panel: "#10121bdd",
  panelBorder: "#ffffff1f",
  menuBackground: "#07070cec",
  button: "#ffffff14",
  buttonHover: "#ffffff30",
  dialogueSize: 25,
  nameSize: 27,
  uiSize: 22,
  radius: 12,
};

// ---------------------------------------------------------------------------
// Elements
// ---------------------------------------------------------------------------

function flatten(children, out = []) {
  for (const child of children) {
    if (Array.isArray(child)) flatten(child, out);
    else if (child != null && child !== false && child !== true)
      out.push(child);
  }
  return out;
}

export const FILL = {
  position: "absolute",
  left: 0,
  top: 0,
  right: 0,
  bottom: 0,
};

/** A flexbox container. `box({ style, onClick, hover, tooltip, key }, ...children)` */
export function box(props = {}, ...children) {
  return { t: "box", ...props, children: flatten(children) };
}

/** A grid container with `columns` equal columns. */
export function grid(columns, props = {}, ...children) {
  return box(
    { ...props, style: { gridColumns: columns, ...props.style } },
    ...children,
  );
}

/** A container that scrolls vertically with the mouse wheel and focus. */
export function scroll(props = {}, ...children) {
  return box(
    {
      ...props,
      style: { overflow: "scroll", flexDirection: "column", ...props.style },
    },
    ...children,
  );
}

/** Plain text. Style props: color, fontSize, fontFamily, fontWeight, italic, lineHeight, textAlign, textShadow. */
export function text(content, props = {}) {
  return { t: "text", text: String(content ?? ""), ...props };
}

/** Text with text tags ({b}, {color=…}, {ruby=…}, …). */
export function richText(markup, props = {}) {
  const size = props.style?.fontSize;
  return {
    t: "text",
    spans: parseMarkup(markup, { baseSize: size }).spans,
    ...props,
  };
}

/** An image from the game directory. `fit`: "cover" | "contain" | "fill". */
export function img(src, props = {}) {
  return { t: "image", src, ...props };
}

/** An image that swaps to `hoverSrc` while hovered or focused and acts as a button. */
export function imageButton(src, hoverSrc, onClick, props = {}) {
  return img(src, { hoverSrc, onClick, ...props });
}

/** A video (H.264 MP4). `loop`, `onEnd`, `fit`. */
export function video(src, props = {}) {
  return { t: "video", src, ...props };
}

/** A horizontal slider. Calls `onChange(value)` while dragged or adjusted with arrow keys. */
export function slider(
  value,
  onChange,
  { min = 0, max = 1, step, ...props } = {},
) {
  return { t: "slider", value, onChange, min, max, step, ...props };
}

/** A single-line text field. Calls `onInput(text)` on edits and `onSubmit(text)` on Enter. */
export function input(
  value,
  onInput,
  { onSubmit, placeholder, maxLength, ...props } = {},
) {
  return {
    t: "input",
    value: String(value ?? ""),
    onInput,
    onSubmit,
    placeholder,
    maxLength,
    ...props,
    style: {
      padding: [8, 12],
      radius: 8,
      background: "#00000066",
      borderWidth: 1,
      borderColor: "#ffffff33",
      ...props.style,
    },
    hover: { borderColor: theme.accent, ...props.hover },
  };
}

/** A clickable box with a text label and hover/focus feedback. */
export function button(label, onClick, props = {}) {
  const { style, hover, textStyle, disabled, ...rest } = props;
  return box(
    {
      style: {
        padding: [10, 24],
        radius: 8,
        background: theme.button,
        justifyContent: "center",
        alignItems: "center",
        color: disabled ? "#ffffff55" : theme.text,
        ...style,
      },
      hover: disabled
        ? undefined
        : { background: theme.buttonHover, color: "#ffffff", ...hover },
      onClick: disabled ? undefined : onClick,
      focusable: !disabled,
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
let hidden = false;
let tooltipText = null;
let tooltipObserved = false;
let sceneLayer = () => null;
const exits = {};

/**
 * Defines (or replaces) a screen.
 * options.z      stacking order, higher is on top
 * options.modal  block clicks and keys from reaching lower screens
 * options.keys   { [key]: (event) => void } handled while the screen is shown
 */
export function screen(name, render, options = {}) {
  screens.set(name, {
    render,
    z: options.z ?? 0,
    modal: !!options.modal,
    keys: options.keys ?? {},
  });
  invalidate();
}

export function showScreen(name, props = {}) {
  shown = shown.filter((s) => s.name !== name);
  shown.push({ name, props, seq: ++showSeq });
  shown.sort(
    (a, b) =>
      (screens.get(a.name)?.z ?? 0) - (screens.get(b.name)?.z ?? 0) ||
      a.seq - b.seq,
  );
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
  return shown
    .filter((s) => filter(s.name))
    .map(({ name, props }) => ({ name, props }));
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

/** Hides every screen to show the scene alone; any click or key brings them back. */
export function setUiHidden(value) {
  hidden = value;
  invalidate();
}

export function isUiHidden() {
  return hidden;
}

/** The tooltip of the hovered or focused element, or null. */
export function tooltip() {
  tooltipObserved = true;
  return tooltipText;
}

// ---------------------------------------------------------------------------
// Rendering and event routing
// ---------------------------------------------------------------------------

function renderRoot() {
  tooltipObserved = false;
  const children = [sceneLayer()];
  if (!hidden) {
    for (const entry of shown) {
      const def = screens.get(entry.name);
      if (!def) continue;
      const content = def.render(entry.props);
      if (def.modal) {
        children.push(
          box(
            {
              key: `screen:${entry.name}`,
              style: FILL,
              onClick: () => {},
              focusable: false,
            },
            content,
          ),
        );
      } else if (content) {
        children.push({
          ...content,
          key: content.key ?? `screen:${entry.name}`,
        });
      }
    }
  }
  return box(
    {
      key: "root",
      style: { ...FILL, fontFamily: theme.font ?? undefined },
      onClick: (e) => emit("backgroundClick", e),
      focusable: false,
    },
    children,
  );
}

onFlush(() => {
  if (!dirty) return;
  dirty = false;
  const tree = renderRoot();
  const options = { instant, exits: { ...exits } };
  instant = false;
  for (const k of Object.keys(exits)) delete exits[k];
  native.ui.commit(tree, options);
});

on("click", (event) => {
  if (hidden) {
    setUiHidden(false);
    return;
  }
  // Only the primary button activates elements; others go to the game (menus).
  if (event.button !== "left") emit("backgroundClick", event);
  else event.handler?.(event);
});

on("handler", (event) => {
  event.handler?.(event.value);
  invalidate();
});

on("tooltip", (event) => {
  const next = event.text ?? null;
  if (next === tooltipText) return;
  tooltipText = next;
  if (tooltipObserved) invalidate();
});

on("key", (event) => {
  if (!event.down) return;
  if (hidden) {
    setUiHidden(false);
    return true;
  }
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
