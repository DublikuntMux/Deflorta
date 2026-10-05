// deflorta/screens — default screens. Games override any of them by calling
// screen(name, render, options) with the same name.

import { command, config, on, setTimer, clearTimer } from "deflorta/core";
import { FILL, box, button, hideScreen, img, invalidate, screen, screenProps, showScreen, text } from "deflorta/ui";
import {
  canSave,
  deleteSave,
  endGame,
  inGame,
  loadGame,
  newGame,
  prefs,
  saveGame,
  saveInfo,
  savePrefs,
} from "deflorta/story";

const ACCENT = "#e8a8c8";
const PANEL = "#10121bdd";

// ---------------------------------------------------------------------------
// Dialogue window
// ---------------------------------------------------------------------------

screen(
  "say",
  ({ who, what, cps }) =>
    box(
      {
        key: "say-window",
        enter: { dur: 0.2, opacity: 0, y: 16 },
        exit: { dur: 0.15, opacity: 0 },
        style: {
          position: "absolute",
          left: 80,
          right: 80,
          bottom: 28,
          minHeight: 180,
          padding: [20, 34],
          gap: 8,
          flexDirection: "column",
          background: PANEL,
          radius: 16,
          borderWidth: 1,
          borderColor: "#ffffff1f",
        },
      },
      who &&
        text(who.name, {
          style: { color: who.color ?? ACCENT, fontSize: 27, fontWeight: 700 },
        }),
      text(what, {
        key: "line",
        cps,
        style: {
          color: "#f3f1f5",
          fontSize: 25,
          lineHeight: 1.45,
          textShadow: { color: "#000000aa", x: 1, y: 2 },
        },
      }),
    ),
  { z: 10 },
);

// ---------------------------------------------------------------------------
// Choices
// ---------------------------------------------------------------------------

screen(
  "choice",
  ({ items }) =>
    box(
      {
        key: "choice",
        enter: { dur: 0.25, opacity: 0 },
        style: { ...FILL, bottom: 220, flexDirection: "column", justifyContent: "center", alignItems: "center", gap: 14 },
      },
      items.map((item, i) =>
        button(item.text, item.select, {
          key: `choice-${i}`,
          style: { width: 640, padding: [14, 24], background: "#141724e6", borderWidth: 1, borderColor: "#ffffff26", radius: 12 },
          hover: { background: "#2a2240f2", borderColor: ACCENT },
          textStyle: { fontSize: 24 },
        }),
      ),
    ),
  { z: 20 },
);

// ---------------------------------------------------------------------------
// Main menu
// ---------------------------------------------------------------------------

function menuButton(label, onClick, disabled = false) {
  return button(label, onClick, {
    disabled,
    style: { width: 260, padding: [12, 20], background: "#00000000", justifyContent: "flex-start", radius: 10 },
    hover: { background: "#ffffff1c", color: ACCENT },
    textStyle: { fontSize: 26 },
  });
}

screen(
  "main_menu",
  () =>
    box(
      { key: "main-menu", style: { ...FILL, background: "#0d0c14" }, exit: { dur: 0.4, opacity: 0 } },
      config.menuBackground && img(config.menuBackground, { fit: "cover", style: FILL }),
      box(
        {
          style: {
            position: "absolute",
            left: 0,
            top: 0,
            bottom: 0,
            width: 420,
            padding: [80, 64],
            gap: 6,
            flexDirection: "column",
            justifyContent: "flex-end",
            background: "#0a0a12c8",
          },
        },
        text(config.title, { style: { fontSize: 52, fontWeight: 700, color: "#ffffff", margin: [0, 0, 36, 0] } }),
        menuButton("Start", () => newGame()),
        menuButton("Load", () => showScreen("game_menu", { page: "load" })),
        menuButton("Preferences", () => showScreen("game_menu", { page: "prefs" })),
        menuButton("Quit", () => command("quit")),
      ),
    ),
  { z: 50 },
);

// ---------------------------------------------------------------------------
// Game menu: save, load, preferences
// ---------------------------------------------------------------------------

const SLOTS = 6;

function formatTime(ms) {
  const d = new Date(ms);
  const pad = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function slotsPage(mode) {
  const slots = [];
  for (let slot = 1; slot <= SLOTS; slot++) {
    const info = saveInfo(slot);
    const onClick =
      mode === "save"
        ? () => {
            if (saveGame(slot)) notify(`Saved to slot ${slot}`);
            invalidate();
          }
        : info && (() => loadGame(slot) || notify("This save cannot be loaded"));
    slots.push(
      box(
        {
          key: `slot-${slot}`,
          onClick,
          hover: onClick ? { background: "#ffffff1c", borderColor: ACCENT } : undefined,
          style: {
            width: 262,
            height: 150,
            padding: 18,
            gap: 6,
            flexDirection: "column",
            background: "#ffffff0d",
            borderWidth: 1,
            borderColor: "#ffffff1a",
            radius: 12,
          },
        },
        text(`Slot ${slot}`, { style: { fontSize: 20, color: ACCENT, fontWeight: 700 } }),
        text(info ? formatTime(info.time) : "Empty", { style: { fontSize: 16, color: "#ffffffaa" } }),
        info && text(info.preview, { style: { fontSize: 17, color: "#ffffffdd" } }),
        info &&
          mode === "save" &&
          box(
            {
              style: { position: "absolute", right: 10, top: 10, padding: [2, 10], radius: 6 },
              hover: { background: "#ff505040" },
              onClick: () => {
                deleteSave(slot);
                invalidate();
              },
            },
            text("✕", { style: { fontSize: 16, color: "#ffffff88" } }),
          ),
      ),
    );
  }
  return box({ style: { flexDirection: "row", flexWrap: "wrap", gap: 18 } }, slots);
}

function choiceRow(label, options, current, apply) {
  return box(
    { style: { flexDirection: "row", alignItems: "center", gap: 10 } },
    text(label, { style: { width: 240, fontSize: 21, color: "#ffffffcc" } }),
    options.map(([name, value]) =>
      button(name, () => apply(value), {
        style: {
          padding: [8, 18],
          background: value === current ? "#e8a8c840" : "#ffffff10",
          borderWidth: 1,
          borderColor: value === current ? ACCENT : "#ffffff1a",
        },
        textStyle: { fontSize: 19 },
      }),
    ),
  );
}

function setPref(name) {
  return (value) => {
    prefs[name] = value;
    savePrefs();
    invalidate();
  };
}

const volumes = [["Off", 0], ["25%", 0.25], ["50%", 0.5], ["80%", 0.8], ["100%", 1]];

function prefsPage() {
  return box(
    { style: { flexDirection: "column", gap: 18 } },
    choiceRow("Text speed", [["Slow", 20], ["Normal", 40], ["Fast", 90], ["Instant", 0]], prefs.textSpeed ?? config.textSpeed, setPref("textSpeed")),
    choiceRow("Auto-forward", [["Off", false], ["On", true]], prefs.autoForward, setPref("autoForward")),
    choiceRow("Music volume", volumes, prefs.musicVolume, setPref("musicVolume")),
    choiceRow("Sound volume", volumes, prefs.soundVolume, setPref("soundVolume")),
    choiceRow("Display", [["Window", false], ["Fullscreen", true]], prefs.fullscreen, setPref("fullscreen")),
  );
}

function navButton(label, onClick, active = false) {
  return button(label, onClick, {
    style: { padding: [10, 22], background: active ? "#e8a8c833" : "#00000000", justifyContent: "flex-start" },
    textStyle: { fontSize: 22, color: active ? ACCENT : undefined },
  });
}

const tab = (label, page, current) => navButton(label, () => showScreen("game_menu", { page }), page === current);

screen(
  "game_menu",
  ({ page }) => {
    const playing = inGame();
    const title = { main: "Paused", save: "Save", load: "Load", prefs: "Preferences" }[page];
    const content = { save: () => slotsPage("save"), load: () => slotsPage("load"), prefs: prefsPage }[page];
    return box(
      { key: "game-menu", enter: { dur: 0.2, opacity: 0 }, style: { ...FILL, flexDirection: "row", background: "#07070cec" } },
      box(
        { style: { width: 300, flexShrink: 0, padding: [70, 30], gap: 4, flexDirection: "column", background: "#ffffff08" } },
        playing && navButton("Return", () => hideScreen("game_menu")),
        playing && tab("Save", "save", page),
        tab("Load", "load", page),
        tab("Preferences", "prefs", page),
        box({ style: { height: 24 } }),
        playing && navButton("Main Menu", () => endGame()),
        navButton(playing ? "Quit" : "Back", () => (playing ? command("quit") : hideScreen("game_menu"))),
      ),
      box(
        { style: { flexGrow: 1, padding: [70, 60], gap: 30, flexDirection: "column" } },
        text(title ?? "", { style: { fontSize: 40, fontWeight: 700, color: "#ffffff" } }),
        content?.(),
        page === "save" && !canSave() && text("The game cannot be saved right now.", { style: { color: "#ffaaaa" } }),
      ),
    );
  },
  {
    z: 100,
    modal: true,
    keys: {
      Escape: () => {
        const page = screenProps("game_menu")?.page;
        if (inGame() && page !== "main") showScreen("game_menu", { page: "main" });
        else hideScreen("game_menu");
      },
    },
  },
);

// ---------------------------------------------------------------------------
// Notifications and errors
// ---------------------------------------------------------------------------

let notifyTimer = null;

/** Shows a short message in the corner of the screen. */
export function notify(message, seconds = 2) {
  showScreen("notify", { message });
  if (notifyTimer) clearTimer(notifyTimer);
  notifyTimer = setTimer(seconds * 1000, () => hideScreen("notify"));
}

screen(
  "notify",
  ({ message }) =>
    box(
      {
        key: "notify",
        enter: { dur: 0.2, opacity: 0, y: -10 },
        exit: { dur: 0.3, opacity: 0 },
        style: { position: "absolute", top: 24, right: 24, padding: [12, 20], radius: 10, background: "#1c1a28ee", borderWidth: 1, borderColor: ACCENT },
      },
      text(message, { style: { fontSize: 19, color: "#ffffff" } }),
    ),
  { z: 500 },
);

screen(
  "error",
  ({ message }) =>
    box(
      { key: "error", style: { ...FILL, padding: 48, gap: 20, flexDirection: "column", background: "#1a0b10f2" } },
      text("Script error", { style: { fontSize: 36, fontWeight: 700, color: "#ff8a9a" } }),
      text(message, { style: { fontSize: 17, color: "#ffe0e4", lineHeight: 1.35 } }),
      box(
        { style: { flexDirection: "row", gap: 12 } },
        button("Main menu", () => {
          hideScreen("error");
          endGame();
        }),
        button("Quit", () => command("quit")),
      ),
    ),
  { z: 1000, modal: true },
);

on("error", (error) => {
  const message = error instanceof Error ? `${error}\n\n${error.stack ?? ""}` : String(error);
  showScreen("error", { message: message.trimEnd() });
});
