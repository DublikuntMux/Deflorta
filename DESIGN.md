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

Non-goals for now: web builds (SpiderMonkey is the native engine), 3D, Live2D.

## Architecture

```
            ┌─────────────────────── game/ (JS, images, audio, fonts) ───────────────────────┐
            │  main.js  ──import──▶  "deflorta" (runtime, embedded in binary)                │
            └────────────────────────────────────────────────────────────────────────────────┘
                     ▲ __deflorta_dispatch(event)          │ __deflorta_pump() → {tree, cmds}
                     │                                     ▼
┌──────────── Rust ──┴─────────────────────────────────────┴──────────────────────────────────┐
│ script.rs   SpiderMonkey host: ES module loader, microtask queue, native __host API        │
│ engine.rs   Engine core: routes input → JS, applies commands, owns timers/UI/audio          │
│ ui/         Retained tree: diff, enter/exit animations, typewriter, taffy flexbox layout,   │
│             hover + click bubbling, draw-list generation                                    │
│ render/     wgpu: instanced SDF quads (rects, rounded corners, borders, images) + glyphon   │
│ audio.rs    kira: music track (streaming, crossfades), sound track                          │
│ assets.rs   sandboxed file access (no path may escape the game dir)                         │
│ app.rs      winit front end          headless.rs  scripted front end (tests, screenshots)    │
└────────────────────────────────────────────────────────────────────────────────────────────┘
```

### Frame and event model

The engine never runs JS per frame. The loop is:

1. An input event or due timer arrives → `Engine` serializes it and calls
   `__deflorta_dispatch(json)`.
2. The microtask queue is drained, so `async` story code runs until its next
   `await` on an engine promise (a line of dialogue, a menu, a pause).
3. `__deflorta_pump()` returns pending output: a new UI tree if anything was
   invalidated, plus commands (timers, music, config, …).
4. Rust diffs the tree, lays it out and redraws. Frames repeat on vsync only
   while an animation or typewriter effect is running; otherwise the process
   sleeps until the next event or timer.

Keeping per-frame work in Rust (animation interpolation, text reveal, hover
styles) means JS cost is proportional to player *actions*, not to frame rate.

### The JS ↔ Rust boundary

The native API is deliberately tiny (`script.rs`):

| `__host.*` | Purpose |
|---|---|
| `log(level, msg)` | logging |
| `readText(path)` | read a text file from the game directory |
| `readData/writeData/deleteData/listData` | per-game user data (saves, prefs) in the OS data dir, written atomically |

Everything else is message passing in JSON:

- **Events** (Rust → JS): `boot`, `click {h, button, revealing}`,
  `key {key, down, repeat, ctrl, shift, alt, revealing}`, `wheel {dy}`,
  `timer {id}`, `revealed`, `quit`.
- **Commands** (JS → Rust): `config`, `timer`, `cancelTimer`, `music`,
  `sound`, `volume`, `revealAll`, `fullscreen`, `quit`.
- **UI tree**: nested `{t: "box"|"text"|"image", key, style, hover, onClick,
  children, text, cps, src, fit, anchor, enter, exit}`. Click handlers are
  replaced by indices when serialized; Rust reports the index back.

JSON was chosen over building JS objects through JSAPI: the trees are small,
SpiderMonkey's `JSON.stringify` and serde are fast, and the boundary stays
trivial to debug and to keep memory-safe.

### Scripting runtime (`runtime/*.js`, embedded)

Games are ES modules. `main.js` imports the public API from `"deflorta"`;
relative imports load other game files. Modules are layered:

| Module | Responsibility |
|---|---|
| `deflorta/core` | host bridge, timers (`setTimeout`), config, storage, event bus, error reporting |
| `deflorta/ui` | elements (`box`, `text`, `img`, `button`), screen stack (z-order, modal, per-screen keys), serialization |
| `deflorta/story` | images/tags, scene state, labels, `say`/`menu`/`pause`, store, rollback, save/load, input bindings, preferences |
| `deflorta/screens` | default screens: dialogue window, choices, main menu, game menu (save/load/prefs), notifications, error screen |
| `deflorta` | public re-exports |

Any default screen is replaced by calling `screen(name, render, options)` with
the same name from game code.

### Story model, saves and rollback

Story code is plain `async` JavaScript:

```js
label("start", async () => {
  scene("bg room", { with: dissolve(1) });
  await eileen`Hello!`;
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

### UI system

- **Layout**: CSS flexbox through `taffy` (`position`, insets, sizes, padding,
  margin, gap, flex direction/wrap/grow/shrink, justify/align). Text nodes are
  measured with cosmic-text; words never break (CSS `overflow-wrap: normal`).
- **Virtual resolution**: games lay out at a fixed size (e.g. 1280×720). The
  engine letterboxes and scales, and shapes text at the physical size so it
  stays crisp at any window size.
- **Text**: inherited `color`, `fontSize`, `fontFamily`, `fontWeight`,
  `italic`, `lineHeight`, `textAlign`, `textShadow`. Fonts come from
  `game/fonts`, so rendering is identical on every machine.
- **Identity**: a node's id is its parent's id plus its `key` (or child index).
  Ids drive animation and typewriter state across re-renders.
- **Animations**: `enter` (from-values) runs when a keyed node appears. When a
  keyed node disappears, its subtree is kept as a *ghost* at its old z-position
  with frozen layout while its `exit` animation plays. The story layer uses this
  for `dissolve`, `moveinright` and friends; the outgoing background is held
  opaque underneath the incoming one for a true crossfade.
- **Input**: hover styles are applied in Rust with no JS round trip; clicks
  bubble from the topmost element to the nearest `onClick`.

### Rendering

- One instanced pipeline draws all rectangles and images: a signed-distance
  rounded box gives anti-aliased corners and borders, and images can be
  rounded too. Consecutive quads with the same texture are batched.
- Text uses glyphon. The draw list is split into *layers* whenever a shape
  follows text, which preserves painter's order with few draw calls.
- Blending happens in sRGB space (non-sRGB targets, glyphon `ColorMode::Web`),
  so translucent UI matches CSS designs.
- Textures load on first use and are evicted after 60 s unused. The adapter is
  requested with `LowPower` and downlevel limits, so it runs on integrated GPUs
  and GL-only hardware.

### Headless mode

`deflorta GAME --test steps.json` plays scripted input (`wait`, `click`, `key`,
`wheel`) against an offscreen renderer and saves screenshots (`shot`). It's
used for regression tests, CI, and store/press screenshots.

## Footprint and performance (measured)

See the README for the current release-binary size. The breakdown is
dominated by SpiderMonkey with JIT and Intl.

- Idle CPU: ~0% (the loop blocks on events and timers).
- Per click: one JSON dispatch plus one tree commit, re-laid-out only when it
  changes.

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

1. **Position tweens** (`move` transition) for keyed nodes whose layout changes.
2. **Image prediction**: decode upcoming images on a worker thread (Ren'Py-style
   lookahead), so large backgrounds never hitch.
3. **Packaging**: `deflorta pack` into a single archive (zip/stored) read via
   `Assets`, plus bytecode/stencil caching for faster startup.
4. **TypeScript declarations** (`deflorta.d.ts`) for editor completion and type
   checking.
5. **Rich text tags** in dialogue (`{b}`, `{color}`, `{w}` pauses) mapped to
   cosmic-text spans.
6. Save thumbnails (offscreen capture already exists), seen-text tracking for
   skip-unread, voice channel, text input, scroll containers with clipping,
   gamepad navigation, hot reload of scripts during development.
