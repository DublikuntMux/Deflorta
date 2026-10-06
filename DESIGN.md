# Deflorta — Design

Deflorta is a visual novel engine in the spirit of Ren'Py: the engine is a small
native core, and **everything a player sees — story, characters, menus, save
screens, preferences — is written in JavaScript** and executed by SpiderMonkey.

## Goals

| Goal | How |
|---|---|
| Cross-platform | Rust + winit (windowing/input), wgpu (Vulkan, Metal, DX12, GL), kira/cpal (audio), SpiderMonkey via `mozjs`. No platform code in the engine itself. |
| High performance | Event-driven loop: zero CPU when idle, JS runs only on input/timers, frames render only while something animates. Layout, animation, text shaping and drawing live in Rust; JS only describes *what* should be on screen. |
| Small footprint | One binary plus the game directory. Only the needed codecs/backends are compiled in. Small saves (~0.5 KB) due to replay-based state. |
| Everything in JS | The native API is a handful of functions. The story runtime, screen system and all default screens are JS modules shipped inside the binary, and games can replace any of them. |

Non-goals for now: web builds (SpiderMonkey is the native engine), 3D, Live2D (probably make own competing standard).

## Architecture

```
    ┌─────────────────────── game/ (JS, images, audio, fonts) ───────────────────────┐
    │  main.js  ──import──▶  "deflorta" (runtime, embedded in binary)                │
    └────────────────────────────────────────────────────────────────────────────────┘
                     ▲ dispatch(event object)             ▼ native modules + ui.commit(tree)
                     │                                     │
┌──────────── Rust ──┴─────────────────────────────────────┴─────────────────────────────────┐
│ script/     SpiderMonkey host: modules, microtasks, typed native calls and direct values   │
│ engine.rs   Engine core: routes input → JS, applies commands, owns timers/UI/audio,        │
│             gates commits on image decoding, captures save thumbnails                      │
│ ui/         Retained tree: diff, enter/exit/move animations, ATL interpreter, typewriter,  │
│             rich text, taffy flex/grid layout, scrolling, focus navigation, widgets        │
│ render/     wgpu: instanced SDF quads (rounded, bordered, rotated, masked, image/video)    │
│             + glyphon text, offscreen capture                                              │
│ video.rs    MP4 demux (mp4) + H.264 decode (OpenH264) on a background thread               │
│ audio.rs    kira: music, sound, voice tracks; video soundtracks                            │
│ assets.rs   sandboxed file access; background image decoding pool                          │
│ app.rs      winit + gilrs front end   headless.rs  scripted front end (tests, screenshots) │
└────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Frame and event model

The engine never runs JS per frame. The loop is:

1. An input event or due timer arrives → `Engine` creates a JS object directly
   and calls the runtime's registered `dispatch(event)` function.
2. The microtask queue is drained, so `async` story code runs until its next
   `await` on an engine promise (a line of dialogue, a menu, a pause).
3. The runtime's registered `flush()` runs its UI and music hooks. Multiple
   invalidations in a turn produce one `native.ui.commit(tree, options)`.
   Native calls read JS values directly and queue typed Rust commands.
4. Rust applies commands in order, reconciles the tree, lays it out and redraws.
   Frames repeat on vsync only
   while an animation or typewriter effect is running; otherwise the process
   sleeps until the next event or timer.

Keeping per-frame work in Rust (animation interpolation, text reveal, hover
styles) means JS cost is proportional to player *actions*, not to frame rate.

### The JS ↔ Rust boundary

The boundary follows React Native's
[New Architecture](https://reactnative.dev/architecture/landing-page): direct
runtime values in place of a serialized message bridge, grouped native modules,
and explicit UI commits. SpiderMonkey's JSAPI provides the direct interface;
Deflorta does not depend on React Native or JSI itself.

`script/mod.rs` owns the runtime, module loader and registered entry points.
`script/native.rs` defines the typed command/event schema and native functions.
`script/value.rs` reads serde types directly from rooted JS values and creates
JS event objects without JSON text. Struct fields match pinned property atoms;
unknown fields are skipped without reading their values.

The `native` export groups engine operations:

| Module | Purpose |
|---|---|
| `log`, `connect` | logging and registration of dispatch/flush callbacks |
| `files` | text files in the game directory |
| `storage` | atomic per-game saves/preferences and a direct array of stored entries |
| `timers` | set and clear engine timers |
| `app` | configuration, fullscreen, quit |
| `audio` | music, sound, voice and channel volumes |
| `ui` | tree commits, reveal, preloading and save thumbnails |

- **Events** (Rust → JS): `boot`, `click {handler, button, revealing}`,
  `handler {handler, value}` (slider changes, text input, video end),
  `key {key, down, repeat, ctrl, shift, alt, revealing}`, `wheel {dy}`,
  `tooltip {text}`, `timer {id}`, `revealed`, `quit`.
- **Native calls** (JS → Rust) read their arguments synchronously. Engine
  mutations are queued as Rust enum values and applied after JS returns,
  avoiding reentrant borrowing of the engine.
- **UI tree**: nested `{t: "box"|"text"|"image"|"slider"|"input"|"video",
  key, style, hover, onClick/onChange/onInput/onSubmit/onEnd, tooltip,
  focusable, autofocus, children, text | spans, cps, src, hoverSrc, fit,
  anchor, enter, exit, move, transform, …}`. Rust reads the tree in place.
  Callback functions stay in a rooted JS array per commit; Rust holds a
  generation and index. Events resolve those handles back to JS functions.
  The displayed tree's callbacks remain alive while a replacement waits for
  images or thumbnail capture. Older callback arrays are released once the
  replacement is shown.

UI commits still read the full element tree. Native layout nodes are reconciled
by identity and element kind, retaining Taffy's layout and measurement caches.
Only changed layout styles, child lists and intrinsic measurement inputs dirty
the affected nodes and their ancestors; paint and handler changes skip layout.
This does not implement Fabric's immutable shadow-node sharing or concurrent
renderer. Layout, text shaping, animation state and rendering remain native.
JSON remains the on-disk save format and is used for story snapshots, outside
the event and UI bridge. Direct arguments are plain data: own enumerable
properties, with `undefined` omitted and non-finite numbers treated as null;
`toJSON` is not invoked.

### Scripting runtime (`runtime/*.js`, embedded)

Games are ES modules. `main.js` imports the public API from `"deflorta"`;
relative imports load other game files. Release builds minify the runtime
with oxc before embedding it; `build.rs` also compiles `src/render/*.wgsl` to
SPIR-V with naga. Modules are layered:

| Module | Responsibility |
|---|---|
| `deflorta/core` | native modules, timers (`setTimeout`), config, storage, event bus, error reporting |
| `deflorta/text` | text-tag parser (`{b}`, `{w}`, `{ruby=…}` → spans), translations (`_()`, `tl/*.json`) |
| `deflorta/ui` | elements and widgets, theme, screen stack (z-order, modal, per-screen keys), tooltips, UI commits |
| `deflorta/scene` | images, layered images, positions, transitions, ATL builder, scene state, music/sound |
| `deflorta/story` | labels, `say`/`menu`/`prompt`/`pause`/`playMovie`, NVL, voice, history, store, persistent data, seen text, rollback, save/load/autosave, input bindings, preferences |
| `deflorta/screens` | default screens: dialogue, NVL, quick menu, choices, input, movie, history, main menu, game menu (paged saves with thumbnails, sliders in preferences), confirm, notifications, tooltips, errors |
| `deflorta` | public re-exports |

Any default screen is replaced by calling `screen(name, render, options)` with
the same name from game code.

### Story model, saves and rollback

Story code is plain `async` JavaScript:

```js
label("start", async () => {
  scene("bg room", { with: dissolve(1) });
  await eileen("Hello!");
  if ((await menu(["Stay", "Leave"])) === "Leave") jump("outside");
});
```

JS closures and suspended async functions cannot be serialized, so Deflorta
saves **inputs, not stacks**:

- Every interaction (`say`, `menu`, `pause`, custom `checkpoint`) is a numbered
  *checkpoint*.
- Entering a label via `jump` starts a new *root*: the engine snapshots `store`,
  the scene (background, sprites, music), shown game screens and the RNG.
- A save is `{root label, root snapshot, inputs given at checkpoints, target
  checkpoint}`.
- **Loading** restores the snapshot and re-runs the label; checkpoints before
  the target resolve instantly with recorded inputs, sound effects are
  suppressed, and the first commit is drawn without transitions.
- **Rollback** is the same operation targeting an earlier checkpoint. Earlier
  roots are kept in memory (64 by default), so rollback crosses `jump`s.

Requirements this places on story code (documented in the README): keep state
in `store`, use `random()`/`randInt()` (seeded, saved), and await only engine
functions. In return, saves are tiny and robust, and rollback is exact.

The same model gives several Ren'Py features almost for free:

- **History** entries remember their root and checkpoint, so clicking a line in
  the backlog is a rollback to that checkpoint. A save stores the history from
  before its root; replay re-adds the rest.
- **NVL pages** are part of the scene snapshot, and each `say` appends to them
  even while fast-forwarding, so pages rebuild themselves on load.
- **Script updates**: a save records the *kind* of each checkpoint. If the
  replayed script asks for something different (or jumps away early), the
  engine notices the mismatch, restarts the scene from its root snapshot and
  tells the player, instead of failing.
- **Autosave** happens at the first interaction of every label and on quit,
  rotating through six slots. Quick save/load use their own slot.
- **Seen text** is a set of hashes of (label, line) in a separate file, so
  skipping can stop at unread text.

### UI system

- **Layout**: CSS flexbox and grid through `taffy` (`position`, insets, sizes,
  padding, margin, gap, flex direction/wrap/grow/shrink, justify/align, equal
  grid tracks, `overflow`). Text nodes are measured with cosmic-text; words
  never break (CSS `overflow-wrap: normal`).
- **Incremental updates**: keyed layout nodes survive reorders and commits.
  Style and content changes invalidate the affected layout caches; unrelated
  subtrees retain theirs. Surface scale changes invalidate text measurements,
  and exit ghosts retain frozen rectangles outside the layout tree. Handler,
  color, opacity and other paint-only changes do not trigger layout.
- **Virtual resolution**: games lay out at a fixed size (e.g. 1280×720). The
  engine letterboxes and scales, and shapes text at the physical size so it
  stays crisp at any window size.
- **Text**: inherited `color`, `fontSize`, `fontFamily`, `fontWeight`,
  `italic`, `lineHeight`, `textAlign`, `textShadow`. Fonts come from
  `game/fonts`, so rendering is identical on every machine. Rich text arrives
  as spans (bold, italic, size, color, font, underline, strikethrough, ruby);
  decorations and ruby annotations are placed from the shaped glyph layout.
  Unchanged text reuses its shaped buffer and measured size without copying
  spans or traversing glyph runs. Shadow and ruby buffers share their owner's
  lifetime, including for elements identified by child index.
- **Typewriter**: spans carry timed waits, click-waits and a fast-forward
  point. A click while typing shows text up to the next click-wait; a click at
  a click-wait resumes typing. The current line is re-shaped only when its
  visible length changes.
- **Identity**: a node's id is its parent's id plus its `key` (or child index).
  Ids drive animation and typewriter state across re-renders.
- **Animations**: `enter` (from-values) runs when a keyed node appears. When a
  keyed node disappears, its subtree is kept as a *ghost* at its old z-position
  with frozen layout while its `exit` animation plays. The story layer uses this
  for `dissolve`, `moveinright` and friends; the outgoing background is held
  opaque underneath the incoming one for a true crossfade. Keyed nodes with
  `move` animate from their previous layout position when it changes.
- **Transforms**: an ATL-style program (`set`, tweens with easing, `pause`,
  `parallel`, `repeat`) is interpreted in Rust each frame. It restarts only when
  the program itself changes, so re-renders don't reset it. Transforms compose
  as similarity transforms (scale, rotation, translation) down the tree.
- **Masks**: transitions can reveal through an image (dark first), a wipe or
  pixellation, evaluated in screen space in the quad shader. Exiting elements
  use the inverted mask.
- **Input**: hover styles are applied in Rust with no JS round trip; clicks
  bubble from the topmost element to the nearest handler. Arrow keys and
  gamepads move focus spatially between focusable elements (nearest in the
  direction pressed), and the focused element uses its `hover` style. Sliders
  drag and step in Rust, text fields edit in Rust (IME commits included), and
  both report changes to their handlers. Scroll containers clip their children
  and scroll with the wheel and focus. Tooltips follow hover and focus.
  The default tooltip uses a `tooltipText` text binding: Rust updates its
  content and visibility in place, so hovering save slots does not re-render
  screens or commit a replacement UI tree. Custom screens that call
  `tooltip()` still invalidate on tooltip changes; unchanged values are ignored.

### Rendering

- One instanced pipeline draws all rectangles and images: a signed-distance
  rounded box gives anti-aliased corners and borders, and images can be
  rounded too. Consecutive quads with the same texture are batched.
- Text uses glyphon. The draw list is split into *layers* whenever a shape
  follows text, which preserves painter's order with few draw calls.
- Blending happens in sRGB space (non-sRGB targets, glyphon `ColorMode::Web`),
  so translucent UI matches CSS designs.
- Images decode on a background pool. A new UI tree is held back (up to 1.5 s)
  until its images are decoded, so a transition never starts with missing
  pictures. `preload()` decodes ahead of time. Textures are evicted after 60 s
  unused. The adapter is requested with `LowPower` and downlevel limits, so it
  runs on integrated GPUs and GL-only hardware.
- Clipping (overflow containers, letterbox) uses per-batch scissor rectangles.
- **Video** frames come from a background decoding thread a few frames ahead,
  paced by the wall clock, and are uploaded into a reused texture. The file's
  AAC soundtrack plays on the music track through kira.
- **Save thumbnails**: opening the game menu or quick-saving asks the front end
  to render the current tree offscreen *before* the menu appears. The capture
  is downscaled and written next to the save when the save happens.

### Headless mode

`deflorta GAME --test steps.json` plays scripted input (`wait`, `move`,
`click`, `release`, `key`, `type`, `wheel`) against an offscreen renderer and
saves screenshots (`shot`). It's used for regression tests, CI, and store/press
screenshots. `tests/demo.json` covers every feature of the demo.

## Footprint and performance (measured)

See the README for the current release-binary size. The breakdown is
dominated by SpiderMonkey with JIT and Intl.

- Idle CPU: ~0% (the loop blocks on events and timers).
- Per click: one direct event dispatch plus a tree commit when invalidated.
  Animation frames do not cross the JS boundary.

## Platform notes

- **Desktop** (Linux, Windows, macOS): supported. `mozjs` downloads a prebuilt
  SpiderMonkey static library when one exists for the target; otherwise it
  builds from source (needs clang and Python).
- **Android**: SpiderMonkey, wgpu, winit and cpal all support it. A thin
  `android-activity` entry point and asset loading from the APK are needed.
- **iOS**: JIT is not allowed. Build `mozjs` without the `jit` feature
  (interpreter + baseline), which is fast enough for VN workloads.

## Roadmap

Next steps, roughly in priority order:

1. **Tooling**: a `deflorta` CLI (`new`, `pack` into a single archive,
   per-platform builds), hot reload that replays to the current line, a
   developer console, and lint (missing labels/images, unserializable state).
2. **TypeScript declarations** (`deflorta.d.ts`) for editor completion and type
   checking.
3. **More video codecs** (VP9/AV1 in WebM) and hardware decoding.
4. Self-voicing (text to speech) and other accessibility options, plus
   Steam/Discord integrations.
