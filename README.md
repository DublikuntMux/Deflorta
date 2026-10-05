# Deflorta

A cross-platform visual novel engine written in Rust and scripted entirely in
JavaScript, from the story to the menus. See
[DESIGN.md](DESIGN.md) for the architecture.

```sh
cargo run --release -- game          # play the demo
cargo run --release -- path/to/game  # play your game
```

Building needs a Rust toolchain and clang (bindgen). A prebuilt SpiderMonkey is
downloaded automatically for common targets.

## A game

```
mygame/
  main.js        entry module
  images/        "bg room" → images/bg room.png unless declared with image()
  audio/
  fonts/         all fonts in here are loaded; pick one with configure({ font })
```

```js
import { configure, character, label, say, scene, show, menu, store, defaults, jump, dissolve } from "deflorta";

configure({ id: "my-game", title: "My Game", width: 1280, height: 720, font: "Noto Sans" });
defaults({ trust: 0 });

const eileen = character("Eileen", { color: "#f4b6d2" });

label("start", async () => {
  scene("bg room", { with: dissolve(1) });
  show("eileen happy");
  await eileen`Hi there!`;
  const answer = await menu("Well?", [["Hello!", "hi"], ["...", "silent"]]);
  if (answer === "hi") store.trust += 1;
  jump("next");
});
```

## Scripting API (`import … from "deflorta"`)

**Story**

| | |
|---|---|
| `label(name, async fn)` | declare a label; `"start"` begins a new game, `"splashscreen"` runs at boot |
| `jump(name)` / `await call(name)` | transfer control / run and return |
| `await say(text)`, `await say(who, text)` | dialogue; `character(name, opts)` returns a speaker usable as `await e("…")` or ``await e`…` `` |
| `await menu(prompt?, choices)` | choices: `"Text"`, `["Text", value]` or `{ text, value, if }` |
| `await pause(seconds?)` | wait for time or a click |
| `checkpoint(kind, present, { record })` | build your own interactions (text input, minigames) |
| `store`, `defaults({...})` | saved game state (JSON-serializable) |
| `random()`, `randInt(a, b)` | deterministic randomness, safe across load/rollback |

**Scene**

| | |
|---|---|
| `image(name, src, { zoom })` | declare an image; the first word of the name is its *tag* |
| `scene(name?, { with })` | clear the scene and set a background |
| `show(name, { at, with, zorder })` / `hide(tag, { with })` | sprites; showing `eileen sad` replaces `eileen happy` |
| `left`, `center`, `right`, `truecenter`, `at(x, y, { zoom })` | positions (anchor-based, like Ren'Py's xalign/yalign) |
| `dissolve(s)`, `fade`, `moveinleft`, `moveinright`, `moveoutleft`, `moveoutright`, `zoomin` | transitions; make your own as `{ dur, in: {opacity, x, y, scale}, out: {...} }` |
| `music.play(file, { loop, fadeIn, fadeOut, volume })`, `music.stop()`, `sound.play(file)` | audio |

**UI** — screens are functions returning elements; call `invalidate()` after
changing state they read (story functions do this for you).

| | |
|---|---|
| `box(props, ...children)`, `text(str, props)`, `img(src, props)`, `button(label, onClick, props)` | elements |
| `screen(name, render, { z, modal, keys })` | define/replace a screen; overriding `"say"`, `"choice"`, `"main_menu"`, `"game_menu"` restyles the game |
| `showScreen(name, props)`, `hideScreen(name)`, `isShown(name)` | screen stack (game screens are saved and rolled back) |
| `notify(message)` | toast |

Element props: `key`, `style`, `hover` (style overrides), `onClick`,
`enter`/`exit` (`{ dur, opacity, x, y, scale }`), images: `fit`
(`fill`/`cover`/`contain`), `anchor: [x, y]`; text: `cps` (typewriter speed).
Style follows CSS flexbox naming: `position`, `left/top/right/bottom`,
`width/height` (px or `"50%"`), `min*/max*`, `padding`/`margin` (n, [v, h] or
[t, r, b, l]), `gap`, `flexDirection`, `flexWrap`, `flexGrow`, `flexShrink`,
`justifyContent`, `alignItems`, `alignSelf`, `display: "none"`, `background`,
`radius`, `borderWidth`, `borderColor`, `opacity`, `scale`, and inherited text
props `color`, `fontSize`, `fontFamily`, `fontWeight`, `italic`, `lineHeight`,
`textAlign`, `textShadow: { color, x, y }`. Colors are `#rgb[a]`/`#rrggbb[aa]`.

**Engine**

`saveGame(slot)`, `loadGame(slot)`, `saveInfo(slot)`, `rollback()`,
`newGame()`, `endGame()`, `prefs` + `savePrefs()`, `keymap`/`actions`,
`on(event, fn)`, `setTimer`/`setTimeout`, `storage`, `readText(path)`,
`config`/`configure()`.

### Rules for story code

Loading and rollback work by re-running the current label with recorded
inputs, so story code must be deterministic:

- Keep game state in `store`, not in module variables.
- Use `random()` instead of `Math.random()`, and don't branch on `Date`.
- Only `await` engine functions (`say`, `menu`, `pause`, `call`, …).

### Controls

Click, Enter or Space advances; mouse wheel up or Page Up rolls back; hold
Ctrl to skip; Esc or right click opens the game menu; F11 toggles fullscreen.

## Automated tests and screenshots

```sh
deflorta game --test steps.json
```

`steps.json` is a list of `{ "wait": ms }`, `{ "click": [x, y], "button": "right" }`,
`{ "key": "Enter" }`, `{ "key": "Control", "down": true }`, `{ "wheel": -1 }` and
`{ "shot": "out.png" }`. Coordinates are in the game's virtual resolution, and
rendering is offscreen (no window). See `tests/demo.json`.

## License

Engine: MIT. Demo fonts: Noto Sans (`game/fonts/LICENSE-noto.txt`).
