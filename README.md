# Deflorta

A cross-platform visual novel engine written in pure Rust and scripted entirely in JavaScript, 
from the story to the menus. See [DESIGN.md](DESIGN.md) for the architecture.

```sh
cargo run --release -- game          # play the demo
cargo run --release -- path/to/game  # play your game
```

Building needs a Rust toolchain and clang (bindgen). 
A prebuilt SpiderMonkey is downloaded automatically for common targets.

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

## Logs

Deflorta logs to stderr with timestamps: engine start-up, the GPU and window,
audio, fonts, script modules, saves and loads, videos, warnings and errors.
Messages from game scripts (`console.log`, `console.warn`, …) appear under
`deflorta::js`. GPU validation errors are logged instead of crashing the game.

```sh
deflorta game 2> deflorta.log                    # default: engine info, warnings, errors
RUST_LOG=deflorta=debug deflorta game 2> deflorta.log   # details for bug reports
RUST_LOG=deflorta=trace,wgpu=warn deflorta game  # everything, including per-frame work
```

When reporting a problem, attach a `RUST_LOG=deflorta=debug` log.

## Automated tests and screenshots

```sh
deflorta game --test tests/demo.json
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
