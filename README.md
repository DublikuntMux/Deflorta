# Deflorta

A cross-platform visual novel engine written in Rust and scripted entirely in JavaScript,
from the story to the menus. See [DESIGN.md](DESIGN.md) for the architecture.

```sh
python3 scripts/build-dist.py
dist/deflorta run game          # play the demo
dist/deflorta run path/to/game  # play your game
```

Building needs a Rust toolchain and clang (bindgen). 
A prebuilt SpiderMonkey is downloaded automatically for common targets.
Linux builds also need the Speech Dispatcher development library (for example,
`libspeechd-dev` on Debian/Ubuntu).

The workspace has four crates: `crates/data` (shared file/archive and module
resolution code), `crates/engine` (the `deflorta` library), `crates/cli` (the
`deflorta` command), and `crates/launcher` (`deflorta-launcher`, shipped to
players). The CLI does not link the engine, renderer or JavaScript VM.

The distribution script builds the CLI in release mode and the game launcher
in both debug and release modes. The release launcher excludes the developer
console and its dependencies; the debug launcher includes it. It assembles this
layout (Windows binaries have an `.exe` suffix):

```text
dist/
  deflorta
  target/
    linux-x86_64/              platform name: <os>-<arch>
      debug/deflorta-launcher
      release/deflorta-launcher
  template/
    game/                     starter files, fonts, license and editor declarations
    runtime/                  JavaScript sources used by analysis and translation tools
```

Keep the whole distribution together; add its root to PATH or move it anywhere.
Templates and runtimes are resolved relative to the CLI executable, independently
of the working directory. `run` and startup checks use the host's debug runtime;
`publish` ships the release runtime. Runtime folders currently hold one executable
and can also hold libraries and engine resources; publishing copies their complete
contents beside `game.dm` and renames the launcher to the game's id.

Use `--output DIR` to choose the distribution folder (an existing folder is
replaced after successful compilation), or `--target RUST_TRIPLE` to build the
distribution for another target with the required Rust target and linker installed.
Cross builds produce a CLI for that target as well. The script requires Python 3.9+
and honors Cargo's target directory. Building individual Cargo binaries does not
assemble their adjacent runtime or template folders.

## CLI workflow

```sh
deflorta create mygame --title "My Game"
deflorta check mygame
deflorta run mygame
deflorta translate update uk -p mygame
deflorta translate status -p mygame
deflorta translate missing uk -p mygame
deflorta bundle mygame                          # build/game.dm
deflorta info mygame/build/game.dm
deflorta publish mygame                         # dist/<os>-<arch>/
deflorta types mygame                           # refresh editor declarations
```

`create` writes a playable story, fonts and their license, `deflorta.d.ts`,
and `jsconfig.json` for editor completion and JavaScript type checking.
`types` refreshes the declarations without replacing an existing editor config.

`check` reports syntax and import errors, missing exports, labels, images,
audio/video files and invalid translation tables. It warns about duplicate or
unused labels, missing fonts and nondeterministic story code. By default it
also boots the scripts without a window to catch startup exceptions;
`--no-boot` selects static checks. Computed values cannot always be checked.

`bundle` resolves static game imports from `main.js`, including default,
named and namespace imports and re-exports. Paths name files exactly:
`./chapter.js`, `../characters.js`, or `/screens.js`; npm package resolution,
dynamic `import()` and TypeScript compilation are not supported. Game scripts
become one `main.js`, minified with oxc; engine imports refer to the runtime
embedded in the launcher. Use `--no-minify` for a readable bundle,
`--emit-js FILE` to inspect it, and `-o FILE` to choose the archive path.
Packing and reading use the pure Rust `lz4r` implementation. `bundle` and
`publish` use LZ4HC at level 9 by default; `--level 2` through `--level 12` select
the HC compression effort, while `--level 1` selects fast compression.

The version 2 `.dm` format has an LZ4-framed index of paths, file sizes and chunk
locations, followed by independent 128 KiB chunks. Each compressed chunk is a
standard LZ4 frame with its decoded size and a content checksum. Already
compressed media and blocks that do not shrink are stored verbatim. The engine
reads and seeks through the archive directly, including streaming movies and
audio. Tooling files and the `build/`, `dist/`, and `node_modules/` directories
are excluded from game assets.
Version 1 archives used raw LZ4 blocks and must be rebuilt with `bundle` or
`publish`; version 2 launchers reject that old format.

`publish` checks the project, bundles it, boots the bundle, and copies the
runtime beside `game.dm`. Players run the named executable; it locates the
archive next to itself, independently of the working directory. Publishing
selects `target/<platform>/release/` next to the CLI. Use `--platform <os>-<arch>`
to select another installed target or `--debug` to ship its debug runtime.
Startup verification always runs with the host's debug runtime, so publishing
for another platform does not try to execute its launcher. Use `-o DIR` and
`--name NAME` to choose the folder and executable name. Distribute the whole folder.

`translate update` extracts dialogue, character names, menu prompts/choices,
input questions, game titles, explicit `_()` strings and the engine interface
strings into `tl/<language>.json`. Existing translations are preserved;
untranslated entries are `null` and show the source text. With no language
arguments it updates all existing tables. `--prune` removes obsolete entries.
Substituted template strings are reported because their final text cannot be
extracted statically. Add languages to `configure({ languages })` to expose
them in preferences, or switch with `setLanguage(id)`.

## A game

```
mygame/
  main.js        entry module
  images/        "bg room" → images/bg room.png unless declared with image()
  audio/
  movies/        H.264 MP4 (AAC audio), VP8/VP9 WebM (Vorbis audio)
  fonts/         all fonts in here are loaded; pick one with configure({ font })
  tl/            translations: tl/<language>.json = { "source": "translation" }
```

```js
import { configure, character, label, say, scene, show, menu, store, defaults, jump, dissolve } from "deflorta";

configure({ id: "my-game", title: "My Game", width: 1280, height: 720, font: "Noto Sans" });
defaults({ trust: 0 });

const eileen = character("Eileen", { color: "#f4b6d2" });

label("start", async () => {
  scene("bg room", { with: dissolve(1) });
  show("eileen happy");
  await eileen`Hi there! {w}Nice to {b}meet{/b} you.`;
  const answer = await menu("Well?", [["Hello!", "hi"], ["...", "silent"]]);
  if (answer === "hi") store.trust += 1;
  jump("next");
});
```

`game/main.js` is a complete demo using most features.

MP4 H.264 playback uses the pure Rust OxideAV decoder, with 8-bit 4:2:0 video,
cropping, presentation-order B-frames and looping. Archive compression uses
`lz4r`; SpiderMonkey remains the JavaScript VM and still requires its native
build toolchain.

WebM files work with `playMovie()`, `video()` and `configure({ menuVideo })`,
including looping and end callbacks. WebM demuxing, VP8/VP9 decoding and color
conversion use pure Rust dependencies; no FFmpeg or libvpx installation is needed.
VP9 supports 8/10/12-bit video with 4:2:0, 4:2:2 or 4:4:4 chroma, displayed as
8-bit RGBA. Vorbis soundtracks stream in mono or stereo; files without audio
play silently. Opus audio, AV1 video and WebM alpha channels are not supported.
WebM soundtracks require the container's duration metadata.

## Scripting API (`import … from "deflorta"`)

**Story**

| | |
|---|---|
| `label(name, async fn)` | declare a label; `"start"` begins a new game, `"splashscreen"` runs at boot |
| `jump(name)` / `await call(name)` | transfer control / run and return |
| `await say(text)`, `await say(who, text, { voice })` | dialogue; `character(name, { color, nvl })` returns a speaker usable as `await e("…")` or ``await e`…` `` |
| `voice(file)` | play a voice file with the next line |
| `nvlNarrator`, `character(name, { nvl: true })`, `nvlClear()` | NVL mode: lines accumulate on a full-screen page |
| `await menu(prompt?, choices)` | choices: `"Text"`, `["Text", value]` or `{ text, value, if }` |
| `await prompt(question, { default, maxLength })` | text input |
| `await pause(seconds?)` | wait for time or a click |
| `await playMovie(src, { skippable })` | full-screen video |
| `checkpoint(kind, present, { record, rollback })` | build your own interactions (minigames) |
| `store`, `defaults({...})` | saved game state (JSON-serializable) |
| `persistent`, `savePersistent()` | data shared by all playthroughs (unlocked endings, gallery) |
| `random()`, `randInt(a, b)` | deterministic randomness, safe across load/rollback |
| `history` | the dialogue backlog |

**Text tags** (in dialogue, menus, `richText`): `{b}`, `{i}`, `{u}`, `{s}`,
`{color=#f88}`, `{size=32}` / `{size=+4}` / `{size=*1.5}`, `{font=Name}`,
`{ruby=furigana}base{/ruby}`, close with `{/b}` etc. Typewriter control:
`{w}` (wait for click), `{w=0.5}` (pause), `{p}` (wait, then line break),
`{nw}` (advance automatically), `{fast}` (show the text before it instantly).
`{{` writes a literal brace.

**Scene**

| | |
|---|---|
| `image(name, src, { zoom })` | declare an image; the first word of the name is its *tag* |
| `layeredImage(tag, layers)` | compose from attribute groups: `show("eileen sad blush")`, `show("eileen -blush")` (see `game/main.js`) |
| `scene(name?, { with })` | clear the scene and set a background |
| `show(name, { at, with, zorder, transform })` / `hide(tag, { with })` | sprites; showing `eileen sad` replaces `eileen happy` |
| `left`, `center`, `right`, `truecenter`, `offscreenleft/right`, `at(x, y, { zoom, rotate })` | positions (anchor-based, like Ren'Py's xalign/yalign) |
| `dissolve`, `fade`, `moveinleft/right`, `moveoutleft/right`, `zoomin` | transitions; custom: `{ dur, ease, in: {opacity, x, y, scale, rotate}, out: {...} }` |
| `move(dur)` | slide a shown image to its new position: `show("eileen", { at: left, with: move() })` |
| `imageDissolve(mask, dur, ramp)`, `wipeleft/right/up/down`, `pixellate(dur, size)` | mask transitions |
| `atl().linear(1, {x: 50}).ease(1, {y: -10}).pause(0.5).repeat()`, `parallel(a, b)`, `shake()`, `bob()` | ATL-style transforms: `x`, `y`, `opacity`, `scale`, `rotate`, `crop`; easings `linear`, `ease`, `easeIn`, `easeOut`, `bounce` |
| `preload(...names)` | decode images ahead of time |
| `music.play(file, { loop, fadeIn, fadeOut, volume })`, `music.stop()`, `sound.play(file)` | audio |

**UI.** Screens are functions returning elements. Call `invalidate()` after
changing state they read (story functions do this for you).

| | |
|---|---|
| `box`, `grid(columns, …)`, `scroll`, `text`, `richText`, `img`, `imageButton(src, hoverSrc, onClick)`, `button(label, onClick)`, `slider(value, onChange, { min, max, step })`, `input(value, onInput, { onSubmit, placeholder, maxLength })`, `video(src, { loop, onEnd })` | elements |
| `screen(name, render, { z, modal, keys })` | define/replace a screen; overriding `say`, `nvl`, `choice`, `input`, `history`, `quick_menu`, `main_menu`, `game_menu` restyles the game |
| `showScreen(name, props)`, `hideScreen(name)`, `isShown(name)` | screen stack (game screens are saved and rolled back) |
| `theme` | colors and sizes used by the default screens |
| `tooltip()`, `notify(message)` | current tooltip text, toast |

Element props: `key`, `style`, `hover` (style overrides while hovered *or
focused*), `onClick`, `tooltip`, `focusable`, `autofocus`, `enter`/`exit`
(`{ dur, ease, opacity, x, y, scale, rotate, mask }`), `move`, `transform`;
images: `fit` (`fill`/`cover`/`contain`), `anchor: [x, y]`; text: `cps`
(typewriter speed), `tooltipText: true` (instant native tooltip text, hidden
when empty); scroll containers: `startAtEnd`.
Use stable `key` values for children that can be inserted, removed or reordered.
Keys preserve native layout caches and interaction/animation state across
renders. Paint and handler changes skip layout; text and size changes invalidate
the affected layout nodes.
Style follows CSS naming: `position`, `left/top/right/bottom`,
`width/height` (px or `"50%"`), `min*/max*`, `padding`/`margin` (n, [v, h] or
[t, r, b, l]), `gap`, `flexDirection`, `flexWrap`, `flexGrow`, `flexShrink`,
`justifyContent`, `alignItems`, `alignSelf`, `gridColumns`, `gridRows`,
`display: "none"`, `overflow: "hidden" | "scroll"`, `background`, `radius`,
`borderWidth`, `borderColor`, `opacity`, `scale`, `rotate`, slider
`fillColor`/`thumbColor`/`thumbSize`, and inherited text props `color`,
`fontSize`, `fontFamily`, `fontWeight`, `italic`, `lineHeight`, `textAlign`,
`textShadow: { color, x, y }`. Colors are `#rgb[a]`/`#rrggbb[aa]`.

**Engine**

`saveGame(slot)`, `loadGame(slot)`, `saveInfo(slot)`, `quickSave()`,
`quickLoad()`, `autosave()`, `rollback()`, `rollbackTo(historyEntry)`,
`newGame()`, `endGame()`, `prefs` + `savePrefs()`, `setLanguage(id)`,
`translations(language, table)`, `_(text)`, `keymap`/`actions`,
`on(event, fn)`, `setTimer`/`setTimeout`, `storage`, `readText(path)`,
`config`/`configure()` (`id`, `title`, `version`, `width`, `height`, `font`,
`textSpeed`, `autosave`, `languages`, `menuBackground`, `menuVideo`).

For low-level engine operations, `native` exposes typed `app`, `audio`, `ui`,
`timers`, `files` and `storage` modules. For example, `native.app.quit()` and
`native.audio.voice(file)` replace the former `command("quit")` and
`command("voice", { file })` message API. Events and UI trees cross directly
as JS values; saves continue to use JSON. See [DESIGN.md](DESIGN.md) for the
boundary and callback lifetime rules.

Saves include a thumbnail, there are 9 pages of slots plus autosave and
quick-save pages, and saves made with an older version of the script resume
at the start of the scene instead of failing.

### Rules for story code

Loading and rollback work by re-running the current label with recorded
inputs, so story code must be deterministic:

- Keep game state in `store`, not in module variables.
- Use `random()` instead of `Math.random()`, and don't branch on `Date`.
- Only `await` engine functions (`say`, `menu`, `pause`, `call`, …).

### Controls

| Action | Keyboard / mouse | Gamepad |
|---|---|---|
| Advance | click, Enter, Space, wheel down | A |
| Roll back | wheel up, Page Up | LB |
| Skip | hold Ctrl, Tab toggles | hold RT |
| History | H | Y |
| Game menu | Esc, right click | B, Start |
| Hide the interface | middle click | |
| Navigate menus | arrow keys + Enter | D-pad / left stick + A |
| Quick save / load | F5 / F9 | |
| Fullscreen | F11 | |
| Self-voicing | F6 | |

### Accessibility

The window exposes the retained UI through `accesskit` and `accesskit_winit`.
Screen readers receive control names, roles, bounds, focus, slider ranges and
text-field values, and can focus, activate and edit controls. Modal screens
expose their own content while open. Dialogue uses polite live regions and
exposes complete text while the typewriter animation runs.

Press **F6** on any game screen to toggle self-voicing, or use **Preferences →
Self-voicing**. The preference is saved. Self-voicing reads dialogue, changed
screen content and hovered or focused controls with the system speech service.
Linux players need Speech Dispatcher running with a configured speech engine;
Windows and macOS use their system voices. Speech initialization failures are
logged and the game continues. Headless tests do not initialize speech.

Use `label` to name sliders, text fields and controls whose visible text is
ambiguous. Set `alt` on image-only controls to describe their action. An empty
`alt` marks a decorative image. Tooltip text supplies a name when other text
is absent. Names should be translated just like visible UI text:

```js
imageButton("images/save.png", "images/save-hover.png", quickSave, { alt: _("Save") });
slider(prefs.musicVolume, value => { prefs.musicVolume = value; savePrefs(); }, { label: _("Music volume") });
```

Set `live: true` on a custom dialogue or status container to announce changes
through the player's screen reader. Self-voicing can also be controlled with
`prefs.selfVoicing` + `savePrefs()` or `actions.selfVoicing()`.

## Logs

Deflorta logs to stderr with timestamps: engine start-up, the GPU and window,
audio, fonts, script modules, saves and loads, videos, warnings and errors.
Messages from game scripts (`console.log`, `console.warn`, …) appear under
`deflorta::js`. GPU validation errors are logged instead of crashing the game.

```sh
deflorta run game 2> deflorta.log                    # default: engine info, warnings, errors
deflorta run game -v 2> deflorta.log                 # debug details for bug reports
RUST_LOG=deflorta=trace,wgpu=warn deflorta run game    # everything, including per-frame work
```

When reporting a problem, attach a `RUST_LOG=deflorta=debug` log.

Debug builds of `deflorta-launcher` open a floating egui developer console.
Press **F12** to hide or reopen it; drag its title bar to move it and its edges
to resize it. It shows engine logs and all JavaScript console messages,
including `console.debug`, with filtering, clearing and automatic scrolling.

```sh
cargo run -p deflorta-launcher -- game
```

Use **Enter** to run a command,
**Shift+Enter** for a new line, and **Up/Down** for command history. Results and
exceptions appear in the log. The console has these diagnostic commands:

| Command | Description |
| --- | --- |
| `help [command]` | List commands and descriptions, or explain one command. |
| `accessibility [--window]` | Inspect the current AccessKit tree, including node IDs, roles, labels, properties and focus. |
| `assets [--window]` | List cached images and their decode/upload states, resident GPU textures, active audio/video, fonts and JavaScript modules. |
| `stats [--window]` | Show process CPU, memory, disk I/O, redraw timings, GPU resources and engine counts. |

Diagnostics print a snapshot by default. For example, `accessibility --window`,
`assets --window` and `stats --window` each open a separate floating egui window
that refreshes every 500 ms. These windows remain open when **F12** hides the
console; close them individually with **×**. The asset window supports filtering,
and the accessibility window shows an expandable tree with node properties.

CPU measurements cover the game process (100% means one CPU core) and need a
second sample after warm-up. Redraw timings include the inspectors. GPU memory
includes wgpu counters and allocation reports where the backend supports them;
image/video texture sizes are RGBA8 estimates. Hardware GPU utilization and
execution time are shown as unavailable. Streamed assets have no full-file
memory estimate.

Other input runs in the live game's JavaScript global scope. Global
declarations persist between commands;
the public game API is available as `deflorta`, for example:

```js
deflorta.store
deflorta.config.textSpeed = 0
deflorta.jump("start")
console.log(deflorta.prefs)
```

Game module locals remain scoped to their modules. Console evaluation drains
promise jobs and commits game updates. The console is excluded from release
builds and does not appear in headless tests or save thumbnails.

## Automated tests and screenshots

```sh
deflorta run game --test tests/demo.json
deflorta run game/build/game.dm --test tests/demo.json  # test the packed game
cargo test --workspace
```

The script is a list of `{ "wait": ms }`, `{ "move": [x, y] }`,
`{ "click": [x, y], "button": "right", "release": false }`, `{ "release": true }`,
`{ "key": "Enter" }`, `{ "key": "Control", "down": true }`, `{ "type": "text" }`,
`{ "wheel": -1 }` and `{ "shot": "out.png" }` steps. Coordinates are in the
game's virtual resolution, and rendering is offscreen (no window).
`tests/demo.json` plays the whole demo and writes 20 screenshots to
`target/shots/`. Run it with a fresh data directory
(`~/.local/share/deflorta/deflorta-demo`) for identical results.
`tests/save-delete.json` covers saving a manual slot, confirming its deletion,
and removing its thumbnail; it expects slot 1 on page 1 to be empty. Set
`XDG_DATA_HOME` to a temporary directory on Linux to isolate test saves.
`tests/save-hover.json` saves the same slot and checks its preview, delete-button
tooltip and tooltip dismissal. With `RUST_LOG=deflorta=trace`, steps 16–28 should
produce no `Committing UI tree` entries.

## License

Engine: MIT. Demo fonts: Noto Sans (`game/fonts/LICENSE-noto.txt`).
