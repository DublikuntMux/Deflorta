// deflorta/screens — default screens. Games restyle them through `theme` or
// replace any of them by calling screen(name, render, options) with the
// same name.

import {
  clearTimer,
  config,
  native,
  on,
  setTimer,
  storage,
} from "deflorta/core";
import {
  FILL,
  box,
  button,
  grid,
  hideScreen,
  img,
  input,
  invalidate,
  richText,
  screen,
  screenProps,
  scroll,
  showScreen,
  slider,
  text,
  theme,
  video,
} from "deflorta/ui";
import { _ } from "deflorta/text";
import {
  AUTOSAVE_SLOTS,
  INSTANT_SPEED,
  actions,
  advance,
  canSave,
  deleteSave,
  endGame,
  history,
  inGame,
  isSkipping,
  loadGame,
  newGame,
  persistent,
  prefs,
  rollbackTo,
  saveGame,
  saveInfo,
  savePrefs,
  setLanguage,
  setNoticeHandler,
  setQuickNotice,
  updatePromptValue,
} from "deflorta/story";

// ---------------------------------------------------------------------------
// Dialogue window and quick menu
// ---------------------------------------------------------------------------

function nameText(who) {
  return text(_(who.name), {
    style: {
      color: who.color ?? theme.accent,
      fontSize: theme.nameSize,
      fontWeight: 700,
    },
  });
}

function dialogueText(markup, props = {}) {
  return richText(markup, {
    ...props,
    style: {
      color: theme.text,
      fontSize: theme.dialogueSize,
      lineHeight: 1.45,
      textShadow: { color: "#000000aa", x: 1, y: 2 },
      ...props.style,
    },
  });
}

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
          bottom: 40,
          minHeight: 180,
          padding: [20, 34],
          gap: 8,
          flexDirection: "column",
          background: theme.panel,
          radius: 16,
          borderWidth: 1,
          borderColor: theme.panelBorder,
        },
      },
      who?.name != null && nameText(who),
      dialogueText(what, { key: "line", cps }),
    ),
  { z: 10 },
);

screen(
  "nvl",
  ({ lines, cps }) =>
    box(
      {
        key: "nvl-window",
        enter: { dur: 0.25, opacity: 0 },
        exit: { dur: 0.2, opacity: 0 },
        style: {
          ...FILL,
          bottom: 40,
          padding: [48, 120],
          gap: 18,
          flexDirection: "column",
          background: "#05060ad0",
        },
      },
      lines.map((line, i) =>
        box(
          { key: `nvl-${i}`, style: { flexDirection: "column", gap: 2 } },
          line.who?.name != null && nameText(line.who),
          dialogueText(_(line.what), {
            cps: i === lines.length - 1 ? cps : undefined,
          }),
        ),
      ),
    ),
  { z: 10 },
);

function quickButton(label, onClick, active = false) {
  return box(
    {
      onClick,
      focusable: false,
      style: {
        padding: [3, 10],
        radius: 6,
        color: active ? theme.accent : "#ffffffaa",
      },
      hover: { color: "#ffffff", background: "#ffffff18" },
    },
    text(label, { style: { fontSize: 15 } }),
  );
}

screen(
  "quick_menu",
  () =>
    box(
      {
        key: "quick-menu",
        style: {
          position: "absolute",
          left: 0,
          right: 0,
          bottom: 6,
          flexDirection: "row",
          justifyContent: "center",
          gap: 6,
        },
      },
      quickButton(_("Back"), () => actions.rollback()),
      quickButton(_("History"), () => actions.history()),
      quickButton(_("Skip"), () => actions.skip(), isSkipping()),
      quickButton(_("Auto"), () => actions.auto(), prefs.autoForward),
      quickButton(_("Save"), () => {
        native.ui.captureThumbnail(false);
        showScreen("game_menu", { page: "save" });
      }),
      quickButton(_("Q.Save"), () => actions.quickSave()),
      quickButton(_("Q.Load"), () => actions.quickLoad()),
      quickButton(_("Prefs"), () => showScreen("game_menu", { page: "prefs" })),
    ),
  { z: 12 },
);

// ---------------------------------------------------------------------------
// Choices, text input and movies
// ---------------------------------------------------------------------------

screen(
  "choice",
  ({ items }) =>
    box(
      {
        key: "choice",
        enter: { dur: 0.25, opacity: 0 },
        style: {
          ...FILL,
          bottom: 230,
          flexDirection: "column",
          justifyContent: "center",
          alignItems: "center",
          gap: 14,
        },
      },
      items.map((item, i) =>
        button(item.text, item.select, {
          key: `choice-${i}`,
          style: {
            width: 640,
            padding: [14, 24],
            background: "#141724e6",
            borderWidth: 1,
            borderColor: "#ffffff26",
            radius: theme.radius,
          },
          hover: { background: "#2a2240f2", borderColor: theme.accent },
          textStyle: { fontSize: 24 },
        }),
      ),
    ),
  { z: 20 },
);

screen(
  "input",
  (props) =>
    box(
      {
        key: "input",
        enter: { dur: 0.2, opacity: 0 },
        style: {
          ...FILL,
          justifyContent: "center",
          alignItems: "center",
          background: "#00000080",
        },
      },
      box(
        {
          style: {
            width: 560,
            padding: 32,
            gap: 18,
            flexDirection: "column",
            background: theme.panel,
            radius: 16,
            borderWidth: 1,
            borderColor: theme.panelBorder,
          },
        },
        text(props.question, { style: { fontSize: 24, color: theme.text } }),
        input(props.value, (value) => updatePromptValue(value), {
          key: "answer",
          autofocus: true,
          maxLength: props.maxLength,
          onSubmit: (value) => props.submit(value),
          style: { fontSize: 22, color: theme.text },
        }),
        box(
          { style: { flexDirection: "row", justifyContent: "flex-end" } },
          button(_("OK"), () => props.submit(props.value)),
        ),
      ),
    ),
  { z: 30, modal: true },
);

screen(
  "movie",
  ({ src, end }) =>
    box(
      {
        key: "movie",
        style: { ...FILL, background: "#000000" },
        onClick: () => advance(),
        focusable: false,
      },
      video(src, { style: FILL, fit: "contain", onEnd: end }),
    ),
  { z: 40 },
);

// ---------------------------------------------------------------------------
// History (backlog)
// ---------------------------------------------------------------------------

screen(
  "history",
  () =>
    box(
      {
        key: "history",
        enter: { dur: 0.2, opacity: 0 },
        style: {
          ...FILL,
          padding: [50, 140],
          gap: 20,
          flexDirection: "column",
          background: theme.menuBackground,
        },
      },
      box(
        {
          style: {
            flexDirection: "row",
            justifyContent: "space-between",
            alignItems: "center",
          },
        },
        text(_("History"), {
          style: { fontSize: 40, fontWeight: 700, color: "#ffffff" },
        }),
        button(_("Return"), () => hideScreen("history")),
      ),
      scroll(
        {
          key: "history-scroll",
          startAtEnd: true,
          style: {
            flexGrow: 1,
            flexShrink: 1,
            gap: 14,
            padding: [0, 16, 0, 0],
          },
        },
        history.length === 0 &&
          text(_("Nothing yet."), { style: { color: theme.mutedText } }),
        history.map((entry, i) =>
          box(
            {
              key: `h-${i}`,
              tooltip: _("Click to return to this line"),
              onClick: () => {
                hideScreen("history");
                rollbackTo(entry);
              },
              style: {
                flexDirection: "column",
                gap: 2,
                padding: [8, 12],
                radius: 8,
                flexShrink: 0,
              },
              hover: { background: "#ffffff12" },
            },
            entry.who &&
              text(_(entry.who.name), {
                style: {
                  color: entry.who.color ?? theme.accent,
                  fontSize: 20,
                  fontWeight: 700,
                },
              }),
            richText(_(entry.what), {
              style: {
                fontSize: 20,
                color: entry.choice ? theme.accent : theme.text,
              },
            }),
            entry.voice &&
              box(
                {
                  onClick: () => native.audio.voice(entry.voice),
                  style: {
                    padding: [2, 8],
                    radius: 6,
                    alignSelf: "flex-start",
                  },
                  hover: { background: "#ffffff20" },
                },
                text(_("Play voice"), {
                  style: { fontSize: 15, color: theme.mutedText },
                }),
              ),
          ),
        ),
      ),
    ),
  {
    z: 90,
    modal: true,
    keys: {
      Escape: () => hideScreen("history"),
      h: () => hideScreen("history"),
    },
  },
);

// ---------------------------------------------------------------------------
// Main menu
// ---------------------------------------------------------------------------

function latestSave() {
  const saves = storage.list().filter((e) => e.name.startsWith("save-"));
  saves.sort((a, b) => b.modified - a.modified);
  return saves[0]?.name.slice("save-".length);
}

function menuButton(label, onClick, disabled = false) {
  return button(label, onClick, {
    disabled,
    style: {
      width: 280,
      padding: [12, 20],
      background: "#00000000",
      justifyContent: "flex-start",
      radius: 10,
    },
    hover: { background: "#ffffff1c", color: theme.accent },
    textStyle: { fontSize: 26 },
  });
}

screen(
  "main_menu",
  () => {
    const latest = latestSave();
    return box(
      {
        key: "main-menu",
        style: { ...FILL, background: "#0d0c14" },
        exit: { dur: 0.4, opacity: 0 },
      },
      config.menuVideo &&
        video(config.menuVideo, { style: FILL, fit: "cover", loop: true }),
      !config.menuVideo &&
        config.menuBackground &&
        img(config.menuBackground, { fit: "cover", style: FILL }),
      box(
        {
          style: {
            position: "absolute",
            left: 0,
            top: 0,
            bottom: 0,
            width: 460,
            padding: [80, 64],
            gap: 6,
            flexDirection: "column",
            justifyContent: "flex-end",
            background: "#0a0a12c8",
          },
        },
        text(_(config.title), {
          style: {
            fontSize: 52,
            fontWeight: 700,
            color: "#ffffff",
            margin: [0, 0, 36, 0],
          },
        }),
        latest && menuButton(_("Continue"), () => loadGame(latest)),
        menuButton(_("Start"), () => newGame()),
        menuButton(_("Load"), () => showScreen("game_menu", { page: "load" })),
        menuButton(_("Preferences"), () =>
          showScreen("game_menu", { page: "prefs" }),
        ),
        menuButton(_("Quit"), () => native.app.quit()),
      ),
    );
  },
  { z: 50 },
);

// ---------------------------------------------------------------------------
// Game menu: save, load, preferences
// ---------------------------------------------------------------------------

const SLOTS_PER_PAGE = 6;
const PAGES = 9;

function formatTime(ms) {
  const d = new Date(ms);
  const pad = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

function slotNames(page) {
  if (page === "auto")
    return Array.from(
      { length: AUTOSAVE_SLOTS },
      (_unused, i) => `auto-${i + 1}`,
    );
  if (page === "quick") return ["quick"];
  return Array.from(
    { length: SLOTS_PER_PAGE },
    (_unused, i) => `${page}-${i + 1}`,
  );
}

function slotLabel(slot) {
  if (slot === "quick") return _("Quick save");
  if (slot.startsWith("auto-")) return `${_("Auto")} ${slot.slice(5)}`;
  return `${_("Slot")} ${slot.split("-")[1]}`;
}

function confirm(message, yes) {
  showScreen("confirm", { message, yes });
}

function slotCard(slot, mode) {
  const info = saveInfo(slot);
  const writable = mode === "save" && !slot.startsWith("auto-");
  const save = () => {
    if (saveGame(slot)) notify(_("Saved"));
    invalidate();
  };
  const onClick =
    mode === "save"
      ? writable &&
        (() => (info ? confirm(_("Overwrite this save?"), save) : save()))
      : info &&
        (() => loadGame(slot) || notify(_("This save cannot be loaded")));
  return box(
    {
      key: `slot-${slot}`,
      onClick: onClick || undefined,
      hover: onClick
        ? { background: "#ffffff1c", borderColor: theme.accent }
        : undefined,
      tooltip: info?.preview,
      style: {
        height: 200,
        padding: 10,
        gap: 4,
        flexDirection: "column",
        background: "#ffffff0d",
        borderWidth: 1,
        borderColor: "#ffffff1a",
        radius: 12,
      },
    },
    box(
      {
        style: {
          height: 126,
          radius: 8,
          background: "#00000066",
          justifyContent: "center",
          alignItems: "center",
        },
      },
      info
        ? img(info.thumbnail, {
            fit: "cover",
            style: { width: "100%", height: "100%", radius: 8 },
          })
        : text(_("Empty"), { style: { fontSize: 16, color: "#ffffff55" } }),
    ),
    text(slotLabel(slot), {
      style: { fontSize: 17, color: theme.accent, fontWeight: 700 },
    }),
    text(info ? formatTime(info.time) : "", {
      style: { fontSize: 14, color: theme.mutedText },
    }),
    info &&
      writable &&
      box(
        {
          style: {
            position: "absolute",
            right: 14,
            top: 14,
            padding: [2, 9],
            radius: 6,
            background: "#00000088",
          },
          hover: { background: "#ff5050aa" },
          tooltip: _("Delete"),
          onClick: () =>
            confirm(_("Delete this save?"), () => deleteSave(slot)),
        },
        text("×", { style: { fontSize: 14, color: "#ffffffcc" } }),
      ),
  );
}

function slotsPage(mode) {
  const page = persistent._savePage ?? "1";
  const pageButton = (id, label) =>
    button(
      label,
      () => {
        persistent._savePage = id;
        invalidate();
      },
      {
        key: `page-${id}`,
        style: {
          padding: [6, 14],
          background: page === id ? "#e8a8c840" : "#ffffff10",
          borderWidth: 1,
          borderColor: page === id ? theme.accent : "#ffffff1a",
        },
        textStyle: { fontSize: 17 },
      },
    );
  return box(
    { style: { flexDirection: "column", gap: 16, flexGrow: 1 } },
    box(
      { style: { flexDirection: "row", gap: 6, flexWrap: "wrap" } },
      pageButton("auto", _("Auto")),
      pageButton("quick", _("Quick")),
      Array.from({ length: PAGES }, (_unused, i) =>
        pageButton(String(i + 1), String(i + 1)),
      ),
    ),
    grid(
      3,
      { style: { gap: 16 } },
      slotNames(page).map((slot) => slotCard(slot, mode)),
    ),
  );
}

function choiceRow(label, options, current, apply) {
  return box(
    {
      style: {
        flexDirection: "row",
        alignItems: "center",
        gap: 10,
        flexShrink: 0,
      },
    },
    text(label, { style: { width: 240, fontSize: 21, color: "#ffffffcc" } }),
    options.map(([name, value]) =>
      button(name, () => apply(value), {
        style: {
          padding: [8, 18],
          background: value === current ? "#e8a8c840" : "#ffffff10",
          borderWidth: 1,
          borderColor: value === current ? theme.accent : "#ffffff1a",
        },
        textStyle: { fontSize: 19 },
      }),
    ),
  );
}

function sliderRow(label, value, display, apply, options) {
  return box(
    {
      style: {
        flexDirection: "row",
        alignItems: "center",
        gap: 16,
        flexShrink: 0,
      },
    },
    text(label, { style: { width: 240, fontSize: 21, color: "#ffffffcc" } }),
    slider(value, apply, {
      ...options,
      style: { width: 380 },
      hover: { thumbColor: theme.accent },
    }),
    text(display, {
      style: { width: 90, fontSize: 18, color: theme.mutedText },
    }),
  );
}

function setPref(name) {
  return (value) => {
    prefs[name] = value;
    savePrefs();
    invalidate();
  };
}

const percent = (v) => `${Math.round(v * 100)}%`;

function prefsPage() {
  const speed = prefs.textSpeed ?? config.textSpeed;
  const languages = config.languages ?? [];
  return scroll(
    { key: "prefs-scroll", style: { gap: 20, flexGrow: 1, flexShrink: 1 } },
    choiceRow(
      _("Display"),
      [
        [_("Window"), false],
        [_("Fullscreen"), true],
      ],
      prefs.fullscreen,
      setPref("fullscreen"),
    ),
    languages.length > 1 &&
      choiceRow(
        _("Language"),
        languages.map((l) => [l.name, l.id ?? null]),
        prefs.language,
        (id) => setLanguage(id),
      ),
    sliderRow(
      _("Text speed"),
      speed,
      speed >= INSTANT_SPEED ? _("Instant") : `${speed} cps`,
      setPref("textSpeed"),
      { min: 10, max: INSTANT_SPEED, step: 10 },
    ),
    choiceRow(
      _("Auto-forward"),
      [
        [_("Off"), false],
        [_("On"), true],
      ],
      prefs.autoForward,
      setPref("autoForward"),
    ),
    sliderRow(
      _("Auto-forward delay"),
      prefs.autoDelay,
      `${prefs.autoDelay.toFixed(1)} s`,
      setPref("autoDelay"),
      { min: 0.5, max: 5, step: 0.5 },
    ),
    choiceRow(
      _("Skip"),
      [
        [_("Seen text"), false],
        [_("All text"), true],
      ],
      prefs.skipUnseen,
      setPref("skipUnseen"),
    ),
    sliderRow(
      _("Music volume"),
      prefs.musicVolume,
      percent(prefs.musicVolume),
      setPref("musicVolume"),
      { min: 0, max: 1, step: 0.05 },
    ),
    sliderRow(
      _("Sound volume"),
      prefs.soundVolume,
      percent(prefs.soundVolume),
      setPref("soundVolume"),
      { min: 0, max: 1, step: 0.05 },
    ),
    sliderRow(
      _("Voice volume"),
      prefs.voiceVolume,
      percent(prefs.voiceVolume),
      setPref("voiceVolume"),
      { min: 0, max: 1, step: 0.05 },
    ),
    choiceRow(
      _("Voice"),
      [
        [_("Stop at next line"), false],
        [_("Keep playing"), true],
      ],
      prefs.voiceSustain,
      setPref("voiceSustain"),
    ),
  );
}

function navButton(label, onClick, active = false) {
  return button(label, onClick, {
    style: {
      padding: [10, 22],
      background: active ? "#e8a8c833" : "#00000000",
      justifyContent: "flex-start",
    },
    textStyle: {
      fontSize: theme.uiSize,
      color: active ? theme.accent : undefined,
    },
  });
}

const tab = (label, page, current) =>
  navButton(label, () => showScreen("game_menu", { page }), page === current);

screen(
  "game_menu",
  ({ page }) => {
    const playing = inGame();
    const title = {
      main: _("Paused"),
      save: _("Save"),
      load: _("Load"),
      prefs: _("Preferences"),
    }[page];
    const content = {
      save: () => slotsPage("save"),
      load: () => slotsPage("load"),
      prefs: prefsPage,
    }[page];
    return box(
      {
        key: "game-menu",
        enter: { dur: 0.2, opacity: 0 },
        style: {
          ...FILL,
          flexDirection: "row",
          background: theme.menuBackground,
        },
      },
      box(
        {
          style: {
            width: 300,
            flexShrink: 0,
            padding: [70, 30],
            gap: 4,
            flexDirection: "column",
            background: "#ffffff08",
          },
        },
        playing && navButton(_("Return"), () => hideScreen("game_menu")),
        playing &&
          navButton(_("History"), () => {
            hideScreen("game_menu");
            showScreen("history");
          }),
        playing && tab(_("Save"), "save", page),
        tab(_("Load"), "load", page),
        tab(_("Preferences"), "prefs", page),
        box({ style: { height: 24 } }),
        playing &&
          navButton(_("Main Menu"), () =>
            confirm(
              _("Return to the main menu? Unsaved progress will be lost."),
              () => endGame(),
            ),
          ),
        navButton(playing ? _("Quit") : _("Back"), () =>
          playing
            ? confirm(_("Quit the game?"), () => native.app.quit())
            : hideScreen("game_menu"),
        ),
      ),
      box(
        {
          style: {
            flexGrow: 1,
            flexShrink: 1,
            padding: [60, 50, 40, 50],
            gap: 24,
            flexDirection: "column",
          },
        },
        text(title ?? "", {
          style: { fontSize: 40, fontWeight: 700, color: "#ffffff" },
        }),
        content?.(),
        page === "save" &&
          !canSave() &&
          text(_("The game cannot be saved right now."), {
            style: { color: "#ffaaaa" },
          }),
      ),
    );
  },
  {
    z: 100,
    modal: true,
    keys: {
      Escape: () => {
        const page = screenProps("game_menu")?.page;
        if (inGame() && page !== "main")
          showScreen("game_menu", { page: "main" });
        else hideScreen("game_menu");
      },
    },
  },
);

screen(
  "confirm",
  ({ message, yes }) =>
    box(
      {
        key: "confirm",
        enter: { dur: 0.15, opacity: 0 },
        style: {
          ...FILL,
          justifyContent: "center",
          alignItems: "center",
          background: "#000000a0",
        },
      },
      box(
        {
          style: {
            width: 520,
            padding: 32,
            gap: 24,
            flexDirection: "column",
            background: theme.panel,
            radius: 16,
            borderWidth: 1,
            borderColor: theme.panelBorder,
          },
        },
        text(message, {
          style: { fontSize: 22, color: theme.text, textAlign: "center" },
        }),
        box(
          {
            style: { flexDirection: "row", justifyContent: "center", gap: 16 },
          },
          button(
            _("Yes"),
            () => {
              hideScreen("confirm");
              yes();
              invalidate();
            },
            { style: { width: 140 } },
          ),
          button(_("No"), () => hideScreen("confirm"), {
            style: { width: 140 },
          }),
        ),
      ),
    ),
  { z: 200, modal: true, keys: { Escape: () => hideScreen("confirm") } },
);

// ---------------------------------------------------------------------------
// Notifications, tooltips and errors
// ---------------------------------------------------------------------------

let notifyTimer = null;

/** Shows a short message in the corner of the screen. */
export function notify(message, seconds = 2) {
  showScreen("notify", { message });
  if (notifyTimer) clearTimer(notifyTimer);
  notifyTimer = setTimer(seconds * 1000, () => hideScreen("notify"));
}

setNoticeHandler((message) => notify(message, 4));
setQuickNotice((message) => notify(message));

screen(
  "notify",
  ({ message }) =>
    box(
      {
        key: "notify",
        enter: { dur: 0.2, opacity: 0, y: -10 },
        exit: { dur: 0.3, opacity: 0 },
        style: {
          position: "absolute",
          top: 24,
          right: 24,
          maxWidth: 520,
          padding: [12, 20],
          radius: 10,
          background: "#1c1a28ee",
          borderWidth: 1,
          borderColor: theme.accent,
        },
      },
      text(message, { style: { fontSize: 19, color: "#ffffff" } }),
    ),
  { z: 500 },
);

screen(
  "tooltip",
  () => {
    return box(
      {
        key: "tooltip",
        style: {
          position: "absolute",
          left: 0,
          right: 0,
          bottom: 34,
          justifyContent: "center",
          flexDirection: "row",
        },
      },
      text("", {
        tooltipText: true,
        style: {
          maxWidth: 800,
          padding: [6, 14],
          radius: 8,
          background: "#000000cc",
          fontSize: 17,
          color: "#ffffff",
        },
      }),
    );
  },
  { z: 950 },
);
showScreen("tooltip");

screen(
  "error",
  ({ message }) =>
    box(
      {
        key: "error",
        style: {
          ...FILL,
          padding: 48,
          gap: 20,
          flexDirection: "column",
          background: "#1a0b10f2",
        },
      },
      text(_("Script error"), {
        style: { fontSize: 36, fontWeight: 700, color: "#ff8a9a" },
      }),
      scroll(
        { style: { flexGrow: 1, flexShrink: 1 } },
        text(message, {
          style: { fontSize: 17, color: "#ffe0e4", lineHeight: 1.35 },
        }),
      ),
      box(
        { style: { flexDirection: "row", gap: 12 } },
        button(_("Main menu"), () => {
          hideScreen("error");
          endGame();
        }),
        button(_("Quit"), () => native.app.quit()),
      ),
    ),
  { z: 1000, modal: true },
);

on("error", (error) => {
  const message =
    error instanceof Error ? `${error}\n\n${error.stack ?? ""}` : String(error);
  showScreen("error", { message: message.trimEnd() });
});
