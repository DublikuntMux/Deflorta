import {
  _, actions, advance, at, center, config, configure, endGame, FILL, inGame, keymap,
  label, left, loadGame, menu, music, newGame, offscreenleft, offscreenright, right,
  nvlClear, nvlNarrator, on, persistent, prefs, prompt, saveGame, saveInfo, savePrefs,
  rollback, say, setLanguage, sound, store, theme, toggleSkip, translations, truecenter,
} from "deflorta";
import { native, reportError, storage } from "deflorta/core";
import { hideScreen, isShown, screen, screenProps, showScreen } from "deflorta/ui";

configure({ id: "runtime-regressions", autosave: false, textSpeed: 0 });

for (const api of [music, sound, left, center, right, truecenter, offscreenleft, offscreenright, FILL]) {
  if (!Object.isFrozen(api) || Reflect.set(api, "added", true))
    throw new Error("engine API or constant is mutable");
  for (const key of Object.keys(api)) {
    if (Reflect.set(api, key, null) || Reflect.deleteProperty(api, key))
      throw new Error(`engine API or constant can change: ${key}`);
  }
}
for (const object of [store, persistent, prefs, theme, keymap, actions, config, at(0.5)]) {
  if (!Reflect.set(object, "_freezeProbe", true) || object._freezeProbe !== true)
    throw new Error("customizable engine state was frozen");
  delete object._freezeProbe;
}

translations("uk", { Hello: "Привіт", Question: "Питання", Left: "Ліворуч", Right: "Праворуч", Wait: "Wait{nw}" });
setLanguage("uk");
for (const key of ["constructor", "toString", "__proto__"]) {
  if (_(key) !== key) throw new Error(`inherited translation: ${key}`);
}
translations("uk", JSON.parse('{"__proto__":"valid own key","invalid":42}'));
setLanguage("uk");
if (_("__proto__") !== "valid own key" || _("invalid") !== "invalid")
  throw new Error("translation values must be own strings");
setLanguage("constructor");
if (_("Hello") !== "Hello") throw new Error("inherited language ID");
translations("constructor", { Hello: "Own language" });
setLanguage("constructor");
if (_("Hello") !== "Own language") throw new Error("own language ID rejected");
setLanguage(null);

label("start", async () => {
  await say(null, "first", { voice: "first.ogg" });
  await say(null, "second", { voice: "second.ogg" });
  await say("third");
});
label("translated", async () => { await say("Hello"); });
label("choices", async () => {
  store.choice = await menu("Question", [
    { label: "Hidden", action: "hidden", if: false },
    { label: "Left", action: "left" },
    { label: "Right", action: "right", if: true },
  ]);
  await say("first");
});
label("choicesNoPrompt", async () => {
  store.choice = await menu([{ label: "Left", action: false }], { nvl: false });
  await say("first");
});
label("nvlChoices", async () => {
  await nvlNarrator("Page");
  store.choice = await menu("Question", [
    { label: "Left", action: "left" },
    { label: "Right", action: "right" },
  ]);
  nvlClear();
  await say("first");
});
label("input", async () => {
  store.answer = await prompt("Question", { default: "Alice" });
  await say("first");
});
label("noWait", async () => {
  await say("Hello{nw}");
  store.finished = true;
  await say("Hello{nw}");
  store.repeated = true;
  await say("last");
});
label("repeated", async () => {
  await say("Same");
  store.first = true;
  await say("Same");
  store.second = true;
  await say("last");
});
label("translatedNoWait", async () => {
  await say("Wait");
  store.finished = true;
  await say("last");
});

on("quit", () => {
  persistent.quitCount = (persistent.quitCount ?? 0) + 1;
  storage.write("quit-count", persistent.quitCount);
});

function action(key) {
  switch (key) {
    case "start": case "translated": case "choices": case "choicesNoPrompt": case "nvlChoices": case "input": case "noWait": case "repeated": case "translatedNoWait":
      newGame(key);
      break;
    case "auto": prefs.autoForward = true; savePrefs(); break;
    case "stop-auto": prefs.autoForward = false; savePrefs(); break;
    case "skip": toggleSkip(true); break;
    case "stop-skip": toggleSkip(false); break;
    case "advance-test": advance(); break;
    case "rollback-test": rollback(); break;
    case "save": saveGame("test", { thumbnail: false }); break;
    case "load": loadGame("test"); break;
    case "end": endGame(); break;
    case "error-test": reportError(new Error("expected checkpoint cancellation")); break;
    case "uk": setLanguage("uk"); break;
    case "source": setLanguage(null); break;
    case "game_menu": case "history": case "confirm": showScreen(key, { page: "save", message: "Confirm" }); break;
    case "close-game_menu": case "close-history": case "close-confirm": hideScreen(key.slice(6)); break;
    case "prepare-quit": case "quit-test":
      configure({ autosave: true });
      persistent.changed = "saved on quit";
      if (key === "quit-test") native.app.quit();
      break;
    case "state":
      const state = JSON.stringify({
        line: screenProps("say")?.what ?? null,
        input: screenProps("input")?.value ?? null,
        choice: store.choice ?? null,
        first: !!store.first, second: !!store.second,
        finished: !!store.finished, repeated: !!store.repeated,
        inGame: inGame(), preview: saveInfo("test")?.preview ?? null,
        blocked: ["game_menu", "history", "confirm"].some(isShown),
      });
      configure({ title: state });
      native.audio.voice(state);
      break;
  }
}

// Keep test controls above modal screens and outside game-state replacement.
// This exercises the normal native key-routing path without bypassing modals.
const keys = ["start", "translated", "choices", "choicesNoPrompt", "nvlChoices", "input", "noWait", "repeated", "translatedNoWait",
  "auto", "stop-auto", "skip", "stop-skip", "advance-test", "rollback-test",
  "save", "load", "end", "error-test", "uk", "source", "game_menu", "history",
  "confirm", "close-game_menu", "close-history", "close-confirm", "prepare-quit", "quit-test", "state"];
screen("tooltip", () => null, {
  z: 1000000,
  keys: Object.fromEntries(keys.map(key => [key, () => action(key)])),
});
showScreen("tooltip");
