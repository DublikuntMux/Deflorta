// deflorta/ui — declarative UI elements, widgets and the screen stack.
//
// Screens are JSX function components. Hook setters schedule rendering;
// external story state uses invalidate(). At the end of the turn the tree is
// committed to the engine, which reads it in place, lays it out
// (flexbox/grid), animates and draws it. Handler functions stay in JS; the
// engine hands them back in click and handler events. Keyboard and
// gamepad focus moves between elements with handlers; focused elements use
// their `hover` style.

import { emit, native, on, onFlush } from "deflorta/core";
import { parseMarkup } from "deflorta/text";
import {
  createElement,
  createRenderer,
  setComponentScheduler,
} from "deflorta/components";

export {
  Fragment,
  createElement,
  useState,
  useReducer,
  useRef,
  useMemo,
  useCallback,
  useEffect,
} from "deflorta/components";

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

export const FILL = {
  position: "absolute",
  left: 0,
  top: 0,
  right: 0,
  bottom: 0,
};

/** A native flexbox container. */
export function View({ children, style, hover, onPress, ...props } = {}) {
  return {
    t: "box",
    ...props,
    onClick: onPress,
    style: mergeStyle(style),
    hover: mergeStyle(hover),
    children,
  };
}

/** A grid container with `columns` equal columns. */
export function Grid({ columns, style, ...props }) {
  return View({
    ...props,
    style: { gridColumns: columns, ...mergeStyle(style) },
  });
}

/** A container that scrolls vertically with the mouse wheel and focus. */
export function ScrollView({ style, ...props } = {}) {
  return View({
    ...props,
    style: {
      overflow: "scroll",
      flexDirection: "column",
      ...mergeStyle(style),
    },
  });
}

/** Plain text. Style props: color, fontSize, fontFamily, fontWeight, italic, lineHeight, textAlign, textShadow. */
export function Text({ children, style, hover, onPress, ...props } = {}) {
  return {
    t: "text",
    text: textContent(children),
    ...props,
    onClick: onPress,
    style: mergeStyle(style),
    hover: mergeStyle(hover),
  };
}

/** Text with text tags ({b}, {color=…}, {ruby=…}, …). */
export function RichText({ children, style, hover, onPress, ...props } = {}) {
  style = mergeStyle(style);
  return {
    t: "text",
    spans: parseMarkup(textContent(children), { baseSize: style?.fontSize })
      .spans,
    ...props,
    onClick: onPress,
    style,
    hover: mergeStyle(hover),
  };
}

/** An image from the game directory. `fit`: "cover" | "contain" | "fill". */
export function Image({ style, hover, onPress, ...props }) {
  return {
    t: "image",
    ...props,
    onClick: onPress,
    style: mergeStyle(style),
    hover: mergeStyle(hover),
  };
}

/** A video (H.264 MP4). `loop`, `onEnd`, `fit`. */
export function Video({ style, hover, onPress, ...props }) {
  return {
    t: "video",
    ...props,
    onClick: onPress,
    style: mergeStyle(style),
    hover: mergeStyle(hover),
  };
}

/** A horizontal slider. Calls onValueChange while dragged or adjusted with arrow keys. */
export function Slider({
  min = 0,
  max = 1,
  onValueChange,
  style,
  hover,
  ...props
}) {
  return {
    t: "slider",
    ...props,
    min,
    max,
    onChange: onValueChange,
    style: mergeStyle(style),
    hover: mergeStyle(hover),
  };
}

/** A controlled single-line field. Calls onChangeText on edits, onSubmit on Enter. */
export function TextInput({ value, onChangeText, style, hover, ...props }) {
  return {
    t: "input",
    ...props,
    value: String(value ?? ""),
    onInput: onChangeText,
    style: {
      padding: [8, 12],
      radius: 8,
      background: "#00000066",
      borderWidth: 1,
      borderColor: "#ffffff33",
      ...mergeStyle(style),
    },
    hover: { borderColor: theme.accent, ...mergeStyle(hover) },
  };
}

/** A clickable container with hover/focus feedback and arbitrary children. */
export function Pressable({
  children,
  onPress,
  style,
  hover,
  disabled,
  ...props
}) {
  return View({
    style: {
      padding: [10, 24],
      radius: 8,
      background: theme.button,
      justifyContent: "center",
      alignItems: "center",
      color: disabled ? "#ffffff55" : theme.text,
      ...mergeStyle(style),
    },
    hover: disabled
      ? undefined
      : {
          background: theme.buttonHover,
          color: "#ffffff",
          ...mergeStyle(hover),
        },
    ...props,
    onPress: disabled ? undefined : onPress,
    focusable: !disabled && (props.focusable ?? true),
    disabled: !!disabled,
    children,
  });
}

/** Style arrays are flattened left to right, like React Native. */
function mergeStyle(style) {
  if (!Array.isArray(style)) return style || undefined;
  return Object.assign({}, ...style.map(mergeStyle));
}

function textContent(children) {
  if (children == null || typeof children === "boolean") return "";
  if (Array.isArray(children)) return children.map(textContent).join("");
  if (typeof children !== "string" && typeof children !== "number") {
    throw new Error("Text and RichText children must be strings or numbers");
  }
  return String(children);
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
const renderer = createRenderer();
setComponentScheduler(invalidate);

function screenRoot(name) {
  return `root/children/${JSON.stringify(["key", `screen:${name}`])}`;
}

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
  emit("screensChanged");
}

export function hideScreen(name) {
  const before = shown.length;
  shown = shown.filter((s) => s.name !== name);
  if (shown.length !== before) {
    renderer.unmount(screenRoot(name));
    invalidate();
    emit("screensChanged");
  }
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
  shown = shown.filter((entry) => {
    if (!filter(entry.name)) return true;
    renderer.unmount(screenRoot(entry.name));
    return false;
  });
  for (const { name, props } of list) showScreen(name, props);
  invalidate();
  emit("screensChanged");
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
      const content = createElement(def.render, {
        ...entry.props,
        key: `screen:${entry.name}`,
      });
      if (def.modal) {
        children.push(
          View({
            key: `screen:${entry.name}`,
            style: FILL,
            onPress: () => {},
            focusable: false,
            modal: true,
            children: content,
          }),
        );
      } else {
        children.push(content);
      }
    }
  }
  return View({
    key: "root",
    style: { ...FILL, fontFamily: theme.font ?? undefined },
    onPress: (e) => emit("backgroundClick", e),
    focusable: false,
    children,
  });
}

onFlush(() => {
  let passes = 0;
  while (dirty) {
    if (++passes > 25)
      throw new Error("Too many UI updates; check effect dependencies");
    dirty = false;
    // Temporarily hiding the interface retains mounted screen state.
    const retainedRoots = hidden
      ? shown.map(({ name }) => screenRoot(name))
      : [];
    const tree = renderer.render(renderRoot(), retainedRoots);
    const options = { instant, exits: { ...exits } };
    native.ui.commit(tree, options);
    instant = false;
    for (const k of Object.keys(exits)) delete exits[k];
    renderer.commit();
  }
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
  // Self-voicing is available on every screen, including modal dialogs.
  if (event.key === "F6") return;
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
