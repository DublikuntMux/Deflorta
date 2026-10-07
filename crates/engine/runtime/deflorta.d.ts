// Type declarations for the Deflorta scripting API.
//
// Games import from "deflorta"; the "deflorta/*" modules expose the runtime's
// building blocks for advanced use. Keep this file next to main.js (the
// `deflorta create` and `deflorta types` commands write it) and add a
// jsconfig.json with "checkJs" for type checking in editors.
//
// Store, persistent data and preferences can be typed per game:
//
//   declare module "deflorta" {
//     interface Store { trust: number; name: string }
//   }

declare module "deflorta" {
  // -------------------------------------------------------------------------
  // Configuration and engine services
  // -------------------------------------------------------------------------

  export interface Language {
    /** Translation id (`tl/<id>.json`); null is the language the script is written in. */
    id: string | null;
    name: string;
  }

  export interface Config {
    /** Save directory name; must be unique per game. */
    id: string;
    title: string;
    /** Shown in logs and stored in saves. */
    version?: unknown;
    /** Virtual resolution; layout happens in these units and is scaled to the window. */
    width: number;
    height: number;
    /** Default font family, loaded from the game's fonts/ directory. */
    font: string;
    /** Characters per second for dialogue; 0 shows text instantly. */
    textSpeed: number;
    /** Delay between lines while skipping, in milliseconds. */
    skipDelay: number;
    clearColor: Color;
    /** Set to false to disable autosaves. */
    autosave?: boolean;
    /** Languages offered in the preferences. */
    languages?: Language[];
    /** Main menu background image. */
    menuBackground?: string;
    /** Main menu background video (loops). */
    menuVideo?: string;
    [option: string]: unknown;
  }

  export const config: Config;
  /** Updates the configuration. Call at the top level of main.js. */
  export function configure(options: Partial<Config>): void;

  /** Logs at info level (shown under `deflorta::js`). */
  export function log(...values: unknown[]): void;

  /** Calls `fn` after `ms` milliseconds; returns an id for clearTimer. */
  export function setTimer(ms: number, fn: () => void): number;
  export function clearTimer(id: number): void;

  export interface StorageEntry {
    name: string;
    /** Milliseconds since the Unix epoch. */
    modified: number;
  }

  /** Per-game JSON storage in the user data directory. Names: letters, digits, `-`, `_`. */
  export const storage: {
    read<T = unknown>(name: string): T | null;
    write(name: string, value: unknown): void;
    remove(name: string): boolean;
    list(): StorageEntry[];
  };

  /** Reads a text file from the game, or null if it does not exist. */
  export function readText(path: string): string | null;

  export interface KeyEvent {
    type: "key";
    key: string;
    down: boolean;
    repeat: boolean;
    ctrl: boolean;
    shift: boolean;
    alt: boolean;
    /** Typewriter text is still appearing. */
    revealing: boolean;
  }

  export interface ClickEvent {
    type: "click";
    handler: ((event: ClickEvent) => void) | null;
    button: "left" | "right" | "middle" | string;
    revealing: boolean;
  }

  export interface WheelEvent {
    type: "wheel";
    dy: number;
    revealing: boolean;
  }

  export interface HandlerEvent {
    type: "handler";
    handler: (value?: number | string) => void;
    value?: number | string;
  }

  export interface TooltipEvent {
    type: "tooltip";
    text: string | null;
  }

  export interface TimerEvent {
    type: "timer";
    id: number;
  }

  export interface EngineEvents {
    boot: { type: "boot" };
    quit: { type: "quit" };
    revealed: { type: "revealed" };
    key: KeyEvent;
    click: ClickEvent;
    wheel: WheelEvent;
    handler: HandlerEvent;
    tooltip: TooltipEvent;
    /** Clicks that no element handled, and non-primary buttons. */
    backgroundClick: ClickEvent;
    /** Errors thrown by game code. */
    error: unknown;
  }

  export type EngineEvent = EngineEvents[Exclude<keyof EngineEvents, "backgroundClick" | "error">] | TimerEvent;

  /**
   * Subscribes to an engine event. Returning `true` stops later listeners.
   * Returns a function that unsubscribes.
   */
  export function on<K extends keyof EngineEvents>(
    type: K,
    fn: (event: EngineEvents[K]) => boolean | void,
  ): () => void;
  export function on(type: string, fn: (event: any) => boolean | void): () => void;

  export interface NativeModules {
    log(level: "error" | "warn" | "info" | "debug" | "trace", message: string): void;
    connect(dispatch: (event: EngineEvent) => void, flush: () => void): void;
    files: { readText(path: string): string | null };
    storage: {
      read(name: string): string | null;
      write(name: string, text: string): void;
      remove(name: string): boolean;
      list(): StorageEntry[];
    };
    timers: { set(id: number, ms: number): void; clear(id: number): void };
    app: { configure(config: Config): void; fullscreen(on: boolean): void; selfVoicing(on: boolean): void; quit(): void };
    audio: {
      music(
        music: { file: string; loop?: boolean; volume?: number } | null,
        fade?: { fadeIn?: number; fadeOut?: number },
      ): void;
      sound(file: string, volume?: number): void;
      /** Plays a voice line (stopping the previous one); null stops voice. */
      voice(file: string | null): void;
      volume(channel: "music" | "sound" | "voice", value: number): void;
    };
    ui: {
      commit(
        tree: Element,
        options?: { instant?: boolean; exits?: Record<string, Animation | null | undefined> },
      ): void;
      /** Shows typewriter text up to the next click-wait. */
      revealSkip(): void;
      preload(images: string[]): void;
      /** Captures a save thumbnail now, or with `after`, once the next tree is shown. */
      captureThumbnail(after?: boolean): void;
      saveThumbnail(name: string): void;
      deleteThumbnail(name: string): void;
    };
  }

  /** Low-level engine modules. */
  export const native: NativeModules;

  // -------------------------------------------------------------------------
  // Text and translation
  // -------------------------------------------------------------------------

  export interface Span {
    text: string;
    b?: boolean;
    i?: boolean;
    u?: boolean;
    s?: boolean;
    color?: Color;
    size?: number;
    font?: string;
    ruby?: string;
    /** Pause in seconds before the text continues ({w=0.5}). */
    wait?: number;
    /** Wait for a click ({w}, {p}). */
    click?: boolean;
    /** Everything before this marker appears instantly ({fast}). */
    fast?: boolean;
  }

  /** Parses text tags (`{b}`, `{color=#f88}`, `{w}`, …) into spans. */
  export function parseMarkup(
    markup: unknown,
    options?: { baseSize?: number },
  ): { spans: Span[]; noWait: boolean };
  /** Text with all tags removed. */
  export function plainText(markup: unknown): string;
  /** Adds translations: `{ "source text": "translated text" }`. */
  export function translations(language: string, table: Record<string, string | null>): void;
  /** Translates into the current language (unchanged when untranslated). */
  export function _(source: string): string;
  /** Strings looked up but missing in the current language. */
  export function missingTranslations(): string[];

  // -------------------------------------------------------------------------
  // UI
  // -------------------------------------------------------------------------

  /** `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`. */
  export type Color = string;
  /** Virtual pixels, `"50%"`, `"12px"` or `"auto"`. */
  export type Dimension = number | `${number}%` | `${number}px` | "auto";
  /** All sides, [vertical, horizontal] or [top, right, bottom, left]. */
  export type Edges = number | [number, number] | [number, number, number, number];
  export type Alignment =
    | "flex-start"
    | "flex-end"
    | "center"
    | "stretch"
    | "baseline"
    | "space-between"
    | "space-around"
    | "space-evenly";
  export type Ease = "linear" | "ease" | "easein" | "easeout" | "easeinout" | "bounce";

  export interface Style {
    display?: "flex" | "grid" | "none";
    position?: "relative" | "absolute";
    left?: Dimension;
    top?: Dimension;
    right?: Dimension;
    bottom?: Dimension;
    width?: Dimension;
    height?: Dimension;
    minWidth?: Dimension;
    minHeight?: Dimension;
    maxWidth?: Dimension;
    maxHeight?: Dimension;
    padding?: Edges;
    margin?: Edges;
    gap?: number;
    flexDirection?: "row" | "column" | "row-reverse" | "column-reverse";
    flexWrap?: "nowrap" | "wrap";
    flexGrow?: number;
    flexShrink?: number;
    justifyContent?: Alignment;
    alignItems?: Alignment;
    alignSelf?: Alignment;
    /** Number of equal grid columns (implies `display: "grid"`). */
    gridColumns?: number;
    gridRows?: number;
    overflow?: "visible" | "hidden" | "scroll";
    background?: Color;
    radius?: number;
    borderWidth?: number;
    borderColor?: Color;
    opacity?: number;
    scale?: number;
    /** Degrees. */
    rotate?: number;
    /** Slider track fill. */
    fillColor?: Color;
    thumbColor?: Color;
    thumbSize?: number;
    scrollbarColor?: Color;
    // Inherited text properties.
    color?: Color;
    fontSize?: number;
    lineHeight?: number;
    fontFamily?: string;
    fontWeight?: number | "normal" | "bold";
    italic?: boolean;
    textAlign?: "left" | "center" | "right" | "justify";
    textShadow?: { color: Color; x?: number; y?: number };
  }

  export type Mask =
    | { kind: "image"; src: string; ramp?: number }
    | { kind: "wipe"; dir?: "left" | "right" | "up" | "down"; ramp?: number }
    | { kind: "pixellate"; size?: number };

  /** An enter/exit/move animation: starting (enter) or final (exit) values. */
  export interface Animation {
    dur: number;
    ease?: Ease;
    opacity?: number;
    x?: number;
    y?: number;
    scale?: number;
    rotate?: number;
    mask?: Mask;
  }

  export interface ElementProps {
    /** Stable identity across renders (animations, layout caches, focus). */
    key?: string;
    style?: Style;
    /** Style overrides while hovered or focused. */
    hover?: Style;
    onClick?: (event: ClickEvent) => void;
    tooltip?: string | null;
    /** Accessible control name. Defaults to text content or image alt text. */
    label?: string;
    /** Politely announce changes to this subtree through the screen reader. */
    live?: boolean;
    focusable?: boolean;
    autofocus?: boolean;
    enter?: Animation;
    exit?: Animation;
    move?: Animation;
    transform?: Atl | null;
  }

  export interface BoxElement extends ElementProps {
    t: "box";
    children: Element[];
    /** Scroll containers start scrolled to the end. */
    startAtEnd?: boolean;
  }

  export interface TextElement extends ElementProps {
    t: "text";
    text?: string;
    spans?: Span[];
    /** Typewriter speed in characters per second. */
    cps?: number;
    /** Shows the hovered element's tooltip, updated natively without re-rendering. */
    tooltipText?: boolean;
  }

  export type Fit = "fill" | "cover" | "contain";

  export interface ImageElement extends ElementProps {
    t: "image";
    src: string;
    /** Describe the image or button action; "" marks a decorative image. */
    alt?: string;
    hoverSrc?: string;
    fit?: Fit;
    /** Point of the image placed at its position, as fractions: [0.5, 1] is bottom center. */
    anchor?: [number, number];
  }

  export interface VideoElement extends ElementProps {
    t: "video";
    src: string;
    loop?: boolean;
    onEnd?: () => void;
    fit?: Fit;
  }

  export interface SliderElement extends ElementProps {
    t: "slider";
    value: number;
    onChange: (value: number) => void;
    min: number;
    max: number;
    step?: number;
  }

  export interface InputElement extends ElementProps {
    t: "input";
    value: string;
    onInput: (text: string) => void;
    onSubmit?: (text: string) => void;
    placeholder?: string;
    maxLength?: number;
  }

  export type Element =
    | BoxElement
    | TextElement
    | ImageElement
    | VideoElement
    | SliderElement
    | InputElement;

  /** Children may be nested arrays; null, undefined and booleans are skipped. */
  export type Child = Element | Child[] | null | undefined | boolean;

  export interface Theme {
    font: string | null;
    accent: Color;
    text: Color;
    mutedText: Color;
    panel: Color;
    panelBorder: Color;
    menuBackground: Color;
    button: Color;
    buttonHover: Color;
    dialogueSize: number;
    nameSize: number;
    uiSize: number;
    radius: number;
  }

  /** Colors and sizes used by the default screens. */
  export const theme: Theme;
  /** Absolute positioning that fills the parent. */
  export const FILL: Style;

  export function box(props?: Omit<Partial<BoxElement>, "t" | "children">, ...children: Child[]): BoxElement;
  /** A grid with `columns` equal columns. */
  export function grid(
    columns: number,
    props?: Omit<Partial<BoxElement>, "t" | "children">,
    ...children: Child[]
  ): BoxElement;
  /** A vertical container that scrolls with the wheel and focus. */
  export function scroll(props?: Omit<Partial<BoxElement>, "t" | "children">, ...children: Child[]): BoxElement;
  export function text(content: unknown, props?: Omit<Partial<TextElement>, "t" | "text">): TextElement;
  /** Text with text tags (`{b}`, `{color=…}`, `{ruby=…}`, …). */
  export function richText(markup: unknown, props?: Omit<Partial<TextElement>, "t" | "spans">): TextElement;
  export function img(src: string, props?: Omit<Partial<ImageElement>, "t" | "src">): ImageElement;
  /** An image button. Set `alt` to a translated description of its action. */
  export function imageButton(
    src: string,
    hoverSrc: string,
    onClick: (event: ClickEvent) => void,
    props?: Omit<Partial<ImageElement>, "t" | "src" | "hoverSrc" | "onClick">,
  ): ImageElement;
  export function video(src: string, props?: Omit<Partial<VideoElement>, "t" | "src">): VideoElement;
  export function slider(
    value: number,
    onChange: (value: number) => void,
    options?: { min?: number; max?: number; step?: number } & Omit<
      Partial<SliderElement>,
      "t" | "value" | "onChange"
    >,
  ): SliderElement;
  export function input(
    value: unknown,
    onInput: (text: string) => void,
    options?: Omit<Partial<InputElement>, "t" | "value" | "onInput">,
  ): InputElement;
  /** A clickable labelled box with hover and focus feedback. */
  export function button(
    label: string,
    onClick: ((event: ClickEvent) => void) | undefined,
    props?: Omit<Partial<BoxElement>, "t" | "children"> & { textStyle?: Style; disabled?: boolean },
  ): BoxElement;

  export interface ScreenOptions {
    /** Stacking order; higher is on top. */
    z?: number;
    /** Blocks clicks and keys from reaching lower screens. */
    modal?: boolean;
    /** Key handlers active while the screen is shown. */
    keys?: Record<string, (event: KeyEvent) => void>;
  }

  /**
   * Defines or replaces a screen. Replacing `say`, `nvl`, `choice`, `input`,
   * `history`, `quick_menu`, `main_menu` or `game_menu` restyles the game.
   */
  export function screen<P = any>(
    name: string,
    render: (props: P) => Element | null | undefined | false,
    options?: ScreenOptions,
  ): void;
  export function showScreen(name: string, props?: unknown): void;
  export function hideScreen(name: string): void;
  export function isShown(name: string): boolean;
  /** Names and props of shown screens accepted by `filter`, bottom to top. */
  export function shownScreens(filter?: (name: string) => boolean): { name: string; props: any }[];
  /** Replaces every shown screen accepted by `filter` with `list`. */
  export function replaceScreens(
    list: { name: string; props?: unknown }[],
    filter?: (name: string) => boolean,
  ): void;
  export function screenProps<P = any>(name: string): P | undefined;
  /** Re-renders screens before the next frame; call after changing state they read. */
  export function invalidate(): void;
  /** Tooltip of the hovered or focused element, or null. */
  export function tooltip(): string | null;
  /** Shows a short message in the corner of the screen. */
  export function notify(message: string, seconds?: number): void;

  // -------------------------------------------------------------------------
  // Scene: images, positions, transitions, transforms, audio
  // -------------------------------------------------------------------------

  /** Places the image's anchor at xalign/yalign of the screen. */
  export interface Position {
    xalign: number;
    yalign: number;
    zoom?: number;
    /** Degrees. */
    rotate?: number;
  }

  export const left: Position;
  export const center: Position;
  export const right: Position;
  export const truecenter: Position;
  export const offscreenleft: Position;
  export const offscreenright: Position;
  export function at(xalign: number, yalign?: number, extra?: { zoom?: number; rotate?: number }): Position;

  type AnimationValues = Omit<Animation, "dur" | "ease">;

  /** How elements enter (`in`: starting values) and leave (`out`: final values). */
  export interface Transition {
    dur: number;
    ease?: Ease;
    in?: AnimationValues;
    out?: AnimationValues;
    /** Slides a shown image to its new position instead. */
    move?: boolean;
  }

  export function dissolve(dur?: number): Transition;
  export function fade(dur?: number): Transition;
  export function moveinleft(dur?: number): Transition;
  export function moveinright(dur?: number): Transition;
  export function moveoutleft(dur?: number): Transition;
  export function moveoutright(dur?: number): Transition;
  export function zoomin(dur?: number): Transition;
  /** Slides a shown image to its new position: `show(name, { at, with: move() })`. */
  export function move(dur?: number, ease?: Ease): Transition;
  /** Reveals in the order of the mask image's brightness (dark first). */
  export function imageDissolve(mask: string, dur?: number, ramp?: number): Transition;
  export function wipeleft(dur?: number): Transition;
  export function wiperight(dur?: number): Transition;
  export function wipeup(dur?: number): Transition;
  export function wipedown(dur?: number): Transition;
  export function pixellate(dur?: number, size?: number): Transition;

  export interface TransformProps {
    x?: number;
    y?: number;
    opacity?: number;
    scale?: number;
    rotate?: number;
    /** Fractional crop rectangle [x, y, w, h]. */
    crop?: [number, number, number, number];
  }

  export type TransformStep =
    | { set: TransformProps }
    | { dur: number; ease?: Ease; to: TransformProps }
    | { pause: number }
    | { parallel: TransformStep[][] }
    | { repeat: boolean | number; steps: TransformStep[] };

  export interface Transform {
    steps: TransformStep[];
  }

  /** An immutable ATL-style animation program; each method returns a new program. */
  export class Atl {
    constructor(steps?: TransformStep[]);
    readonly steps: TransformStep[];
    set(props: TransformProps): Atl;
    tween(dur: number, props: TransformProps, ease?: Ease): Atl;
    linear(dur: number, props: TransformProps): Atl;
    ease(dur: number, props: TransformProps): Atl;
    easeIn(dur: number, props: TransformProps): Atl;
    easeOut(dur: number, props: TransformProps): Atl;
    bounce(dur: number, props: TransformProps): Atl;
    pause(seconds: number): Atl;
    /** Appends another program. */
    after(program: Atl): Atl;
    /** Repeats the program `times` times, or forever. */
    repeat(times?: number | true): Atl;
    toJSON(): Transform;
  }

  export function atl(): Atl;
  /** Runs programs at the same time. */
  export function parallel(...programs: Atl[]): Atl;
  export function shake(strength?: number, dur?: number): Atl;
  export function bob(height?: number, period?: number): Atl;

  /**
   * Declares an image. The first word of the name is its tag: showing
   * "eileen happy" replaces a shown "eileen …". Undeclared names resolve to
   * `images/<name>.png`.
   */
  export function image(name: string, src: string, options?: { zoom?: number }): void;

  export type Layer =
    | { src: string }
    | { group: string; options: Record<string, string>; default?: string }
    | { attribute: string; src: string };

  /** Declares an image composed from attribute groups: `show("eileen sad blush")`. */
  export function layeredImage(tag: string, layers: Layer[], options?: { zoom?: number }): void;

  export interface ShowOptions {
    at?: Position;
    with?: Transition;
    zorder?: number;
    /** A transform program, or null to remove the current one. */
    transform?: Atl | null;
  }

  /** Shows an image, replacing the image with the same tag. `-attribute` removes a layered attribute. */
  export function show(name: string, options?: ShowOptions): void;
  /** Hides the image with the given tag. */
  export function hide(name: string, options?: { with?: Transition }): void;
  /** Clears the scene, optionally showing a background, and hides the dialogue window. */
  export function scene(name?: string | null, options?: { with?: Transition }): void;
  /** Decodes images ahead of time. */
  export function preload(...names: string[]): void;

  /** Background music; saved with the scene. */
  export const music: {
    play(
      file: string,
      options?: { loop?: boolean; fadeIn?: number; fadeOut?: number; volume?: number },
    ): void;
    stop(options?: { fadeOut?: number }): void;
  };

  /** One-shot sound effects; skipped while fast-forwarding a load or rollback. */
  export const sound: {
    play(file: string, options?: { volume?: number }): void;
  };

  // -------------------------------------------------------------------------
  // Story
  // -------------------------------------------------------------------------

  /** Game variables; augment to type them. Must stay JSON-serializable. */
  export interface Store {
    [key: string]: any;
  }
  /** Data shared by all playthroughs; augment to type it. */
  export interface Persistent {
    [key: string]: any;
  }

  export const store: Store;
  /** Declares default store values, applied when a new game starts. */
  export function defaults(values: Partial<Store>): void;
  export const persistent: Persistent;
  export function savePersistent(): void;

  export interface Prefs {
    textSpeed: number | null;
    autoForward: boolean;
    /** Seconds before auto-forward advances. */
    autoDelay: number;
    musicVolume: number;
    soundVolume: number;
    voiceVolume: number;
    /** Keep voice playing into the next line. */
    voiceSustain: boolean;
    /** Read dialogue and controls with the system speech service (F6 toggles). */
    selfVoicing: boolean;
    skipUnseen: boolean;
    fullscreen: boolean;
    language: string | null;
  }

  export const prefs: Prefs;
  /** Saves and applies `prefs`. */
  export function savePrefs(): void;
  /** Switches the language; null is the language the script is written in. */
  export function setLanguage(language: string | null): void;

  /** A random number in [0, 1) that replays identically after loading and rollback. */
  export function random(): number;
  /** A random integer in [min, max]. */
  export function randInt(min: number, max: number): number;

  /** Declares a label. `"start"` begins a new game; `"splashscreen"` runs at boot. */
  export function label(name: string, fn: (...args: any[]) => Promise<unknown> | unknown): void;
  /** Transfers control to another label, ending the current one. */
  export function jump(name: string): never;
  /** Runs another label and returns when it finishes. */
  export function call(name: string, ...args: unknown[]): Promise<unknown>;

  export interface Who {
    name: string | null;
    color?: Color;
    /** Lines go to the full-screen NVL page. */
    nvl?: boolean;
    [prop: string]: unknown;
  }

  export interface SayOptions {
    /** A voice file to play with the line. */
    voice?: string;
    [prop: string]: unknown;
  }

  /** A speaker: `await eileen("Hi!")` or ``await eileen`Hi!` ``. */
  export interface Speaker {
    (text: string, options?: SayOptions): Promise<void>;
    (strings: TemplateStringsArray, ...values: unknown[]): Promise<void>;
    readonly who: Who;
  }

  /** Creates a speaker. Extra options reach custom say screens. */
  export function character(
    name: string | null,
    options?: { color?: Color; nvl?: boolean; [prop: string]: unknown },
  ): Speaker;
  /** The narrator on the NVL page. */
  export const nvlNarrator: Speaker;
  export function nvlClear(): void;
  /** Plays a voice file with the next line. */
  export function voice(file: string): void;

  /** Narrates a line, or shows a line by a speaker, and waits for the player. */
  export function say(text: string): Promise<void>;
  export function say(who: string | Who | null, text: string, options?: SayOptions): Promise<void>;

  /** Waits for `seconds`, or for a click when omitted. */
  export function pause(seconds?: number): Promise<void>;

  export type Choice<T> =
    | string
    | [text: string, value?: T]
    | { text: string; value?: T; if?: unknown };

  /** Presents choices and resolves with the chosen value. */
  export function menu<T = string>(choices: Choice<T>[]): Promise<T>;
  export function menu<T = string>(prompt: string | null, choices: Choice<T>[]): Promise<T>;

  /** Asks the player to type text; resolves with the trimmed answer. */
  export function prompt(
    question: string,
    options?: { default?: string; maxLength?: number; allowEmpty?: boolean },
  ): Promise<string>;

  /** Plays a full-screen video until it ends or, if skippable, the player clicks. */
  export function playMovie(src: string, options?: { skippable?: boolean }): Promise<void>;

  export interface Checkpoint<T> {
    kind: string;
    index: number;
    resolve(value?: T): void;
    cleanup: (() => void) | null;
    [prop: string]: unknown;
  }

  /**
   * Suspends the story until the player responds, for custom interactions.
   * With `record`, the resolved value is saved and replayed on load.
   */
  export function checkpoint<T = void>(
    kind: string,
    present: (pending: Checkpoint<T>) => void,
    options?: { record?: boolean; rollback?: boolean },
  ): Promise<T>;

  /** Hides the dialogue window until the next line. */
  export function windowHide(): void;

  export interface HistoryEntry {
    who: { name: string | null; color?: Color } | null;
    what: string;
    voice?: string | null;
    choice?: boolean;
    root?: number;
    index: number;
  }

  /** The dialogue backlog, oldest first. */
  export const history: HistoryEntry[];

  /** True while a game is in progress. */
  export function inGame(): boolean;
  export function isSkipping(): boolean;
  /** Starts or stops skip mode. */
  export function toggleSkip(on?: boolean): void;
  /** Continues past the current line, pause or movie. */
  export function advance(): void;
  /** Starts a new game at `start` (default "start"). */
  export function newGame(start?: string): void;
  /** Leaves the current game for the main menu. */
  export function endGame(): void;
  /** Steps back to the previous line or choice. */
  export function rollback(): boolean;
  /** Rolls back to a backlog entry. */
  export function rollbackTo(entry: HistoryEntry): boolean;

  /** True when the story is waiting for the player and can be saved. */
  export function canSave(): boolean;
  /** Saves to a slot name (letters, digits, `-`, `_`), with a thumbnail by default. */
  export function saveGame(slot: string, options?: { thumbnail?: boolean }): boolean;
  export function loadGame(slot: string): boolean;
  /** Save metadata, or null for an empty slot. */
  export function saveInfo(slot: string): { time: number; preview: string; thumbnail: string } | null;
  export function deleteSave(slot: string): void;
  export function quickSave(): boolean;
  export function quickLoad(): boolean;
  /** Saves to the oldest autosave slot. */
  export function autosave(options?: { now?: boolean }): boolean;

  export type ActionName =
    | "advance"
    | "rollback"
    | "menu"
    | "history"
    | "skip"
    | "auto"
    | "hideUi"
    | "quickSave"
    | "quickLoad"
    | "fullscreen"
    | "selfVoicing";

  /** Key bindings: key name → action. */
  export const keymap: Record<string, ActionName | string>;
  /** Actions that keys and the default screens trigger. */
  export const actions: Record<ActionName, (event?: KeyEvent | ClickEvent | WheelEvent) => void> &
    Record<string, (event?: unknown) => void>;
}

declare module "deflorta/core" {
  export {
    native,
    log,
    config,
    configure,
    setTimer,
    clearTimer,
    storage,
    readText,
    on,
  } from "deflorta";
  /** Calls listeners of `type`; returns true when one handled the event. */
  export function emit(type: string, event?: unknown): boolean;
  /** Logs an error and shows the error screen. */
  export function reportError(error: unknown): void;
  /** Runs `fn` at the end of every turn to commit output. */
  export function onFlush(fn: () => void): void;
}

declare module "deflorta/text" {
  export { parseMarkup, plainText, translations, _, missingTranslations } from "deflorta";
  /** Switches the translation table (null: source language). */
  export function useLanguage(language: string | null): void;
}

declare module "deflorta/ui" {
  export {
    theme,
    FILL,
    box,
    grid,
    scroll,
    text,
    richText,
    img,
    imageButton,
    video,
    slider,
    input,
    button,
    screen,
    showScreen,
    hideScreen,
    isShown,
    shownScreens,
    replaceScreens,
    screenProps,
    invalidate,
    tooltip,
  } from "deflorta";
  import type { Animation, Element } from "deflorta";
  /** The next commit skips enter/exit animations. */
  export function markInstant(): void;
  /** Plays `spec` when the element with `key` disappears in the next commit. */
  export function exitWith(key: string, spec: Animation | undefined): void;
  /** Installs the renderer of the scene below all screens. */
  export function setSceneLayer(render: () => Element | null): void;
  /** Hides all screens until the next click or key. */
  export function setUiHidden(hidden: boolean): void;
  export function isUiHidden(): boolean;
}

declare module "deflorta/scene" {
  export {
    image,
    layeredImage,
    left,
    center,
    right,
    truecenter,
    offscreenleft,
    offscreenright,
    at,
    dissolve,
    fade,
    moveinleft,
    moveinright,
    moveoutleft,
    moveoutright,
    zoomin,
    move,
    imageDissolve,
    wipeleft,
    wiperight,
    wipeup,
    wipedown,
    pixellate,
    Atl,
    atl,
    parallel,
    shake,
    bob,
    show,
    hide,
    preload,
    music,
    sound,
  } from "deflorta";
  import type { Position, Transition, Atl } from "deflorta";

  export interface Sprite {
    tag: string;
    name: string;
    attrs?: string[];
    at: Position;
    zorder: number;
    transform?: ReturnType<Atl["toJSON"]> | null;
  }

  /** Serializable scene state, saved and rolled back with the store. */
  export const scene: {
    bg: string | null;
    sprites: Sprite[];
    music: { file: string; loop: boolean; volume: number } | null;
    nvl: { who: { name: string | null; color?: string } | null; what: string }[];
  };
  export function resetScene(): void;
  export function restoreScene(data: unknown): void;
  export function setReplayCheck(fn: () => boolean): void;
  /** Clears the scene and optionally shows a background (keeps the dialogue window). */
  export function setScene(name?: string | null, options?: { with?: Transition }): void;
  /** Image files an image name needs. */
  export function imageSources(name: string): string[];
}

declare module "deflorta/story" {
  export {
    store,
    defaults,
    persistent,
    savePersistent,
    prefs,
    savePrefs,
    setLanguage,
    random,
    randInt,
    label,
    jump,
    call,
    history,
    inGame,
    checkpoint,
    isSkipping,
    toggleSkip,
    character,
    nvlNarrator,
    voice,
    nvlClear,
    say,
    advance,
    pause,
    menu,
    prompt,
    playMovie,
    scene as sceneStatement,
    windowHide,
    newGame,
    endGame,
    rollback,
    rollbackTo,
    canSave,
    saveGame,
    loadGame,
    saveInfo,
    deleteSave,
    quickSave,
    quickLoad,
    autosave,
    keymap,
    actions,
  } from "deflorta";
  /** Text speed slider maximum; at this value text appears instantly. */
  export const INSTANT_SPEED: number;
  /** Characters per second for dialogue (0 = instant). */
  export function textSpeed(): number;
  /** Screens managed by the runtime; other shown screens are saved with the game. */
  export const SYSTEM_SCREENS: Set<string>;
  export const SAVE_VERSION: number;
  export const AUTOSAVE_SLOTS: number;
  export function setNoticeHandler(fn: (message: string) => void): void;
  export function setQuickNotice(fn: (message: string) => void): void;
  export function updatePromptValue(value: string): void;
}

declare module "deflorta/screens" {
  export { notify } from "deflorta";
}

// Globals provided by the engine.
declare var console: {
  log(...values: unknown[]): void;
  info(...values: unknown[]): void;
  debug(...values: unknown[]): void;
  warn(...values: unknown[]): void;
  error(...values: unknown[]): void;
};
declare function setTimeout<A extends unknown[]>(fn: (...args: A) => void, ms?: number, ...args: A): number;
declare function clearTimeout(id: number): void;
