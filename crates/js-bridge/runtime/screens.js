import {
  config,
  native,
  on,
  storage,
} from "deflorta/core";
import {
  FILL,
  View,
  Pressable,
  Grid,
  ScrollView,
  Text,
  RichText,
  Image,
  TextInput,
  Slider,
  Video,
  hideScreen,
  invalidate,
  screen,
  screenProps,
  showScreen,
  theme,
  useState,
} from "deflorta/ui";
import { _ } from "deflorta/text";
import { notify } from "deflorta/notifications";
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

function Button({ children, textStyle, ...props }) {
  return (
    <Pressable {...props}>
      <Text style={textStyle}>{children}</Text>
    </Pressable>
  );
}

function NameText({ who }) {
  return (
    <Text
      style={{
        color: who.color ?? theme.accent,
        fontSize: theme.nameSize,
        fontWeight: 700,
      }}
    >
      {_(who.name)}
    </Text>
  );
}

function DialogueText({ children, style, ...props }) {
  return (
    <RichText
      {...props}
      style={{
        color: theme.text,
        fontSize: theme.dialogueSize,
        lineHeight: 1.45,
        textShadow: { color: "#000000aa", x: 1, y: 2 },
        ...style,
      }}
    >
      {children}
    </RichText>
  );
}

screen(
  "say",
  ({ who, what, cps, revealKey }) => (
    <View
      key="say-window"
      live
      enter={{ dur: 0.2, opacity: 0, y: 16 }}
      exit={{ dur: 0.15, opacity: 0 }}
      style={{
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
      }}
    >
      {who?.name != null && <NameText who={who} />}
      <DialogueText key={`line:${revealKey ?? "custom"}`} cps={cps}>
        {_(what)}
      </DialogueText>
    </View>
  ),
  { z: 10 },
);

screen(
  "nvl",
  ({ lines, cps, revealKey }) => (
    <View
      key="nvl-window"
      enter={{ dur: 0.25, opacity: 0 }}
      exit={{ dur: 0.2, opacity: 0 }}
      style={{
        ...FILL,
        bottom: 40,
        padding: [48, 120],
        gap: 18,
        flexDirection: "column",
        background: "#05060ad0",
      }}
    >
      {lines.map((line, i) => (
        <View
          key={`nvl-${i}`}
          live={i === lines.length - 1}
          style={{ flexDirection: "column", gap: 2 }}
        >
          {line.who?.name != null && <NameText who={line.who} />}
          <DialogueText
            key={
              i === lines.length - 1 ? `line:${revealKey ?? "custom"}` : "line"
            }
            cps={i === lines.length - 1 ? cps : undefined}
          >
            {_(line.what)}
          </DialogueText>
        </View>
      ))}
    </View>
  ),
  { z: 10 },
);

function QuickButton({ children, onPress, active = false }) {
  return (
    <View
      onPress={onPress}
      focusable={false}
      style={{
        padding: [3, 10],
        radius: 6,
        color: active ? theme.accent : "#ffffffaa",
      }}
      hover={{ color: "#ffffff", background: "#ffffff18" }}
    >
      <Text style={{ fontSize: 15 }}>{children}</Text>
    </View>
  );
}

screen(
  "quick_menu",
  () => (
    <View
      key="quick-menu"
      style={{
        position: "absolute",
        left: 0,
        right: 0,
        bottom: 6,
        flexDirection: "row",
        justifyContent: "center",
        gap: 6,
      }}
    >
      <QuickButton onPress={() => actions.rollback()}>{_("Back")}</QuickButton>
      <QuickButton onPress={() => actions.history()}>
        {_("History")}
      </QuickButton>
      <QuickButton onPress={() => actions.skip()} active={isSkipping()}>
        {_("Skip")}
      </QuickButton>
      <QuickButton onPress={() => actions.auto()} active={prefs.autoForward}>
        {_("Auto")}
      </QuickButton>
      <QuickButton
        onPress={() => {
          native.ui.captureThumbnail(false);
          showScreen("game_menu", { page: "save" });
        }}
      >
        {_("Save")}
      </QuickButton>
      <QuickButton onPress={() => actions.quickSave()}>
        {_("Q.Save")}
      </QuickButton>
      <QuickButton onPress={() => actions.quickLoad()}>
        {_("Q.Load")}
      </QuickButton>
      <QuickButton onPress={() => showScreen("game_menu", { page: "prefs" })}>
        {_("Prefs")}
      </QuickButton>
    </View>
  ),
  { z: 12 },
);

screen(
  "choice",
  ({ items }) => (
    <View
      key="choice"
      enter={{ dur: 0.25, opacity: 0 }}
      style={{
        ...FILL,
        bottom: 230,
        flexDirection: "column",
        justifyContent: "center",
        alignItems: "center",
        gap: 14,
      }}
    >
      {items.map((item, i) => (
        <Button
          key={`choice-${i}`}
          onPress={item.select}
          style={{
            width: 640,
            padding: [14, 24],
            background: "#141724e6",
            borderWidth: 1,
            borderColor: "#ffffff26",
            radius: theme.radius,
          }}
          hover={{ background: "#2a2240f2", borderColor: theme.accent }}
          textStyle={{ fontSize: 24 }}
        >
          {_(item.text)}
        </Button>
      ))}
    </View>
  ),
  { z: 20 },
);

screen(
  "input",
  (props) => (
    <View
      key="input"
      enter={{ dur: 0.2, opacity: 0 }}
      style={{
        ...FILL,
        justifyContent: "center",
        alignItems: "center",
        background: "#00000080",
      }}
    >
      <View
        style={{
          width: 560,
          padding: 32,
          gap: 18,
          flexDirection: "column",
          background: theme.panel,
          radius: 16,
          borderWidth: 1,
          borderColor: theme.panelBorder,
        }}
      >
        <Text style={{ fontSize: 24, color: theme.text }}>
          {_(props.question)}
        </Text>
        <TextInput
          key="answer"
          value={props.value}
          onChangeText={updatePromptValue}
          label={_(props.question)}
          autofocus
          maxLength={props.maxLength}
          onSubmit={props.submit}
          style={{ fontSize: 22, color: theme.text }}
        />
        <View style={{ flexDirection: "row", justifyContent: "flex-end" }}>
          <Button onPress={() => props.submit(props.value)}>{_("OK")}</Button>
        </View>
      </View>
    </View>
  ),
  { z: 30, modal: true },
);

screen(
  "movie",
  ({ src, end }) => (
    <View
      key="movie"
      style={{ ...FILL, background: "#000000" }}
      onPress={() => advance()}
      focusable={false}
    >
      <Video src={src} style={FILL} fit="contain" onEnd={end} />
    </View>
  ),
  { z: 40 },
);

screen(
  "history",
  () => (
    <View
      key="history"
      enter={{ dur: 0.2, opacity: 0 }}
      style={{
        ...FILL,
        padding: [50, 140],
        gap: 20,
        flexDirection: "column",
        background: theme.menuBackground,
      }}
    >
      <View
        style={{
          flexDirection: "row",
          justifyContent: "space-between",
          alignItems: "center",
        }}
      >
        <Text style={{ fontSize: 40, fontWeight: 700, color: "#ffffff" }}>
          {_("History")}
        </Text>
        <Button onPress={() => hideScreen("history")}>{_("Return")}</Button>
      </View>
      <ScrollView
        key="history-scroll"
        startAtEnd
        style={{ flexGrow: 1, flexShrink: 1, gap: 14, padding: [0, 16, 0, 0] }}
      >
        {history.length === 0 && (
          <Text style={{ color: theme.mutedText }}>{_("Nothing yet.")}</Text>
        )}
        {history.map((entry, i) => (
          <View
            key={`h-${i}`}
            tooltip={_("Click to return to this line")}
            onPress={() => {
              hideScreen("history");
              rollbackTo(entry);
            }}
            style={{
              flexDirection: "column",
              gap: 2,
              padding: [8, 12],
              radius: 8,
              flexShrink: 0,
            }}
            hover={{ background: "#ffffff12" }}
          >
            {entry.who && (
              <Text
                style={{
                  color: entry.who.color ?? theme.accent,
                  fontSize: 20,
                  fontWeight: 700,
                }}
              >
                {_(entry.who.name)}
              </Text>
            )}
            <RichText
              style={{
                fontSize: 20,
                color: entry.choice ? theme.accent : theme.text,
              }}
            >
              {_(entry.what)}
            </RichText>
            {entry.voice && (
              <View
                onPress={() => native.audio.voice(entry.voice)}
                style={{ padding: [2, 8], radius: 6, alignSelf: "flex-start" }}
                hover={{ background: "#ffffff20" }}
              >
                <Text style={{ fontSize: 15, color: theme.mutedText }}>
                  {_("Play voice")}
                </Text>
              </View>
            )}
          </View>
        ))}
      </ScrollView>
    </View>
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

function latestSave() {
  const saves = storage.list().filter((e) => e.name.startsWith("save-"));
  saves.sort((a, b) => b.modified - a.modified);
  return saves[0]?.name.slice("save-".length);
}

function MenuButton(props) {
  return (
    <Button
      {...props}
      style={{
        width: 280,
        padding: [12, 20],
        background: "#00000000",
        justifyContent: "flex-start",
        radius: 10,
      }}
      hover={{ background: "#ffffff1c", color: theme.accent }}
      textStyle={{ fontSize: 26 }}
    />
  );
}

screen(
  "main_menu",
  () => {
    const latest = latestSave();
    return (
      <View
        key="main-menu"
        style={{ ...FILL, background: "#0d0c14" }}
        exit={{ dur: 0.4, opacity: 0 }}
      >
        {config.menuVideo && (
          <Video src={config.menuVideo} style={FILL} fit="cover" loop />
        )}
        {!config.menuVideo && config.menuBackground && (
          <Image src={config.menuBackground} fit="cover" style={FILL} />
        )}
        <View
          style={{
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
          }}
        >
          <Text
            style={{
              fontSize: 52,
              fontWeight: 700,
              color: "#ffffff",
              margin: [0, 0, 36, 0],
            }}
          >
            {_(config.title)}
          </Text>
          {latest && (
            <MenuButton onPress={() => loadGame(latest)}>
              {_("Continue")}
            </MenuButton>
          )}
          <MenuButton onPress={() => newGame()}>{_("Start")}</MenuButton>
          <MenuButton onPress={() => showScreen("game_menu", { page: "load" })}>
            {_("Load")}
          </MenuButton>
          <MenuButton
            onPress={() => showScreen("game_menu", { page: "prefs" })}
          >
            {_("Preferences")}
          </MenuButton>
          <MenuButton onPress={() => native.app.quit()}>{_("Quit")}</MenuButton>
        </View>
      </View>
    );
  },
  { z: 50 },
);

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

function SlotCard({ slot, mode }) {
  const info = saveInfo(slot);
  const writable = mode === "save" && !slot.startsWith("auto-");
  const save = () => {
    if (saveGame(slot)) notify(_("Saved"));
    invalidate();
  };
  const onPress =
    mode === "save"
      ? writable &&
        (() => (info ? confirm(_("Overwrite this save?"), save) : save()))
      : info &&
        (() => loadGame(slot) || notify(_("This save cannot be loaded")));
  return (
    <View
      key={`slot-${slot}`}
      onPress={onPress || undefined}
      tooltip={info?.preview}
      hover={
        onPress
          ? { background: "#ffffff1c", borderColor: theme.accent }
          : undefined
      }
      style={{
        height: 200,
        padding: 10,
        gap: 4,
        flexDirection: "column",
        background: "#ffffff0d",
        borderWidth: 1,
        borderColor: "#ffffff1a",
        radius: 12,
      }}
    >
      <View
        style={{
          height: 126,
          radius: 8,
          background: "#00000066",
          justifyContent: "center",
          alignItems: "center",
        }}
      >
        {info ? (
          <Image
            src={info.thumbnail}
            fit="cover"
            style={{ width: "100%", height: "100%", radius: 8 }}
          />
        ) : (
          <Text style={{ fontSize: 16, color: "#ffffff55" }}>{_("Empty")}</Text>
        )}
      </View>
      <Text style={{ fontSize: 17, color: theme.accent, fontWeight: 700 }}>
        {slotLabel(slot)}
      </Text>
      <Text style={{ fontSize: 14, color: theme.mutedText }}>
        {info ? formatTime(info.time) : ""}
      </Text>
      {info && writable && (
        <View
          style={{
            position: "absolute",
            right: 14,
            top: 14,
            padding: [2, 9],
            radius: 6,
            background: "#00000088",
          }}
          hover={{ background: "#ff5050aa" }}
          tooltip={_("Delete")}
          label={`${_("Delete")}: ${slotLabel(slot)}`}
          onPress={() =>
            confirm(_("Delete this save?"), () => deleteSave(slot))
          }
        >
          <Text style={{ fontSize: 14, color: "#ffffffcc" }}>×</Text>
        </View>
      )}
    </View>
  );
}

function SlotsPage({ mode }) {
  const [page, setPage] = useState(() => persistent._savePage ?? "1");
  const selectPage = (id) => {
    persistent._savePage = id;
    setPage(id);
  };
  const pageButton = (id, label) => (
    <Button
      key={`page-${id}`}
      onPress={() => selectPage(id)}
      style={{
        padding: [6, 14],
        background: page === id ? "#e8a8c840" : "#ffffff10",
        borderWidth: 1,
        borderColor: page === id ? theme.accent : "#ffffff1a",
      }}
      textStyle={{ fontSize: 17 }}
    >
      {label}
    </Button>
  );
  return (
    <View style={{ flexDirection: "column", gap: 16, flexGrow: 1 }}>
      <View style={{ flexDirection: "row", gap: 6, flexWrap: "wrap" }}>
        {pageButton("auto", _("Auto"))}
        {pageButton("quick", _("Quick"))}
        {Array.from({ length: PAGES }, (_unused, i) =>
          pageButton(String(i + 1), String(i + 1)),
        )}
      </View>
      <Grid columns={3} style={{ gap: 16 }}>
        {slotNames(page).map((slot) => (
          <SlotCard key={`slot-${slot}`} slot={slot} mode={mode} />
        ))}
      </Grid>
    </View>
  );
}

function ChoiceRow({ label, options, current, apply }) {
  return (
    <View
      style={{
        flexDirection: "row",
        alignItems: "center",
        gap: 10,
        flexShrink: 0,
      }}
    >
      <Text style={{ width: 240, fontSize: 21, color: "#ffffffcc" }}>
        {label}
      </Text>
      {options.map(([name, value]) => (
        <Button
          key={name}
          onPress={() => apply(value)}
          label={`${label}: ${name}`}
          style={{
            padding: [8, 18],
            background: value === current ? "#e8a8c840" : "#ffffff10",
            borderWidth: 1,
            borderColor: value === current ? theme.accent : "#ffffff1a",
          }}
          textStyle={{ fontSize: 19 }}
        >
          {name}
        </Button>
      ))}
    </View>
  );
}

function SliderRow({ label, value, display, apply, ...options }) {
  return (
    <View
      style={{
        flexDirection: "row",
        alignItems: "center",
        gap: 16,
        flexShrink: 0,
      }}
    >
      <Text style={{ width: 240, fontSize: 21, color: "#ffffffcc" }}>
        {label}
      </Text>
      <Slider
        {...options}
        label={label}
        value={value}
        onValueChange={apply}
        style={{ width: 380 }}
        hover={{ thumbColor: theme.accent }}
      />
      <Text style={{ width: 90, fontSize: 18, color: theme.mutedText }}>
        {display}
      </Text>
    </View>
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

function PrefsPage() {
  const speed = prefs.textSpeed ?? config.textSpeed;
  const languages = config.languages ?? [];
  return (
    <ScrollView
      key="prefs-scroll"
      style={{ gap: 20, flexGrow: 1, flexShrink: 1 }}
    >
      {native.app.platform() !== "android" && (
        <ChoiceRow
          label={_("Display")}
          options={[
            [_("Window"), false],
            [_("Fullscreen"), true],
          ]}
          current={prefs.fullscreen}
          apply={setPref("fullscreen")}
        />
      )}
      {languages.length > 1 && (
        <ChoiceRow
          label={_("Language")}
          options={languages.map((l) => [l.name, l.id ?? null])}
          current={prefs.language}
          apply={setLanguage}
        />
      )}
      <SliderRow
        label={_("Text speed")}
        value={speed}
        display={speed >= INSTANT_SPEED ? _("Instant") : `${speed} cps`}
        apply={setPref("textSpeed")}
        min={10}
        max={INSTANT_SPEED}
        step={10}
      />
      <ChoiceRow
        label={_("Auto-forward")}
        options={[
          [_("Off"), false],
          [_("On"), true],
        ]}
        current={prefs.autoForward}
        apply={setPref("autoForward")}
      />
      <SliderRow
        label={_("Auto-forward delay")}
        value={prefs.autoDelay}
        display={`${prefs.autoDelay.toFixed(1)} s`}
        apply={setPref("autoDelay")}
        min={0.5}
        max={5}
        step={0.5}
      />
      <ChoiceRow
        label={_("Self-voicing")}
        options={[
          [_("Off"), false],
          [_("On"), true],
        ]}
        current={prefs.selfVoicing}
        apply={setPref("selfVoicing")}
      />
      <ChoiceRow
        label={_("Skip")}
        options={[
          [_("Seen text"), false],
          [_("All text"), true],
        ]}
        current={prefs.skipUnseen}
        apply={setPref("skipUnseen")}
      />
      <SliderRow
        label={_("Music volume")}
        value={prefs.musicVolume}
        display={percent(prefs.musicVolume)}
        apply={setPref("musicVolume")}
        min={0}
        max={1}
        step={0.05}
      />
      <SliderRow
        label={_("Sound volume")}
        value={prefs.soundVolume}
        display={percent(prefs.soundVolume)}
        apply={setPref("soundVolume")}
        min={0}
        max={1}
        step={0.05}
      />
      <SliderRow
        label={_("Voice volume")}
        value={prefs.voiceVolume}
        display={percent(prefs.voiceVolume)}
        apply={setPref("voiceVolume")}
        min={0}
        max={1}
        step={0.05}
      />
      <ChoiceRow
        label={_("Voice")}
        options={[
          [_("Stop at next line"), false],
          [_("Keep playing"), true],
        ]}
        current={prefs.voiceSustain}
        apply={setPref("voiceSustain")}
      />
    </ScrollView>
  );
}

function NavButton({ children, active = false, ...props }) {
  return (
    <Button
      {...props}
      style={{
        padding: [10, 22],
        background: active ? "#e8a8c833" : "#00000000",
        justifyContent: "flex-start",
      }}
      textStyle={{
        fontSize: theme.uiSize,
        color: active ? theme.accent : undefined,
      }}
    >
      {children}
    </Button>
  );
}

function Tab({ children, page, current }) {
  return (
    <NavButton
      onPress={() => showScreen("game_menu", { page })}
      active={page === current}
    >
      {children}
    </NavButton>
  );
}

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
    return (
      <View
        key="game-menu"
        enter={{ dur: 0.2, opacity: 0 }}
        style={{
          ...FILL,
          flexDirection: "row",
          background: theme.menuBackground,
        }}
      >
        <View
          style={{
            width: 300,
            flexShrink: 0,
            padding: [70, 30],
            gap: 4,
            flexDirection: "column",
            background: "#ffffff08",
          }}
        >
          {playing && (
            <NavButton onPress={() => hideScreen("game_menu")}>
              {_("Return")}
            </NavButton>
          )}
          {playing && (
            <NavButton
              onPress={() => {
                hideScreen("game_menu");
                showScreen("history");
              }}
            >
              {_("History")}
            </NavButton>
          )}
          {playing && (
            <Tab page="save" current={page}>
              {_("Save")}
            </Tab>
          )}
          <Tab page="load" current={page}>
            {_("Load")}
          </Tab>
          <Tab page="prefs" current={page}>
            {_("Preferences")}
          </Tab>
          <View style={{ height: 24 }} />
          {playing && (
            <NavButton
              onPress={() =>
                confirm(
                  _("Return to the main menu? Unsaved progress will be lost."),
                  () => endGame(),
                )
              }
            >
              {_("Main Menu")}
            </NavButton>
          )}
          <NavButton
            onPress={() =>
              playing
                ? confirm(_("Quit the game?"), () => native.app.quit())
                : hideScreen("game_menu")
            }
          >
            {playing ? _("Quit") : _("Back")}
          </NavButton>
        </View>
        <View
          style={{
            flexGrow: 1,
            flexShrink: 1,
            padding: [60, 50, 40, 50],
            gap: 24,
            flexDirection: "column",
          }}
        >
          <Text style={{ fontSize: 40, fontWeight: 700, color: "#ffffff" }}>
            {title ?? ""}
          </Text>
          {(page === "save" || page === "load") && <SlotsPage mode={page} />}
          {page === "prefs" && <PrefsPage />}
          {page === "save" && !canSave() && (
            <Text style={{ color: "#ffaaaa" }}>
              {_("The game cannot be saved right now.")}
            </Text>
          )}
        </View>
      </View>
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
  ({ message, yes }) => (
    <View
      key="confirm"
      enter={{ dur: 0.15, opacity: 0 }}
      style={{
        ...FILL,
        justifyContent: "center",
        alignItems: "center",
        background: "#000000a0",
      }}
    >
      <View
        style={{
          width: 520,
          padding: 32,
          gap: 24,
          flexDirection: "column",
          background: theme.panel,
          radius: 16,
          borderWidth: 1,
          borderColor: theme.panelBorder,
        }}
      >
        <Text style={{ fontSize: 22, color: theme.text, textAlign: "center" }}>
          {message}
        </Text>
        <View
          style={{ flexDirection: "row", justifyContent: "center", gap: 16 }}
        >
          <Button
            onPress={() => {
              hideScreen("confirm");
              yes();
              invalidate();
            }}
            style={{ width: 140 }}
          >
            {_("Yes")}
          </Button>
          <Button onPress={() => hideScreen("confirm")} style={{ width: 140 }}>
            {_("No")}
          </Button>
        </View>
      </View>
    </View>
  ),
  { z: 200, modal: true, keys: { Escape: () => hideScreen("confirm") } },
);

setNoticeHandler((message) => notify(message, { state: "error", duration: 4 }));
setQuickNotice((message) => notify(message));

screen(
  "tooltip",
  () => (
    <View
      key="tooltip"
      style={{
        position: "absolute",
        left: 0,
        right: 0,
        bottom: 34,
        justifyContent: "center",
        flexDirection: "row",
      }}
    >
      <Text
        tooltipText
        style={{
          maxWidth: 800,
          padding: [6, 14],
          radius: 8,
          background: "#000000cc",
          fontSize: 17,
          color: "#ffffff",
        }}
      />
    </View>
  ),
  { z: 950 },
);
showScreen("tooltip");

screen(
  "error",
  ({ message }) => (
    <View
      key="error"
      style={{
        ...FILL,
        padding: 48,
        gap: 20,
        flexDirection: "column",
        background: "#1a0b10f2",
      }}
    >
      <Text style={{ fontSize: 36, fontWeight: 700, color: "#ff8a9a" }}>
        {_("Script error")}
      </Text>
      <ScrollView style={{ flexGrow: 1, flexShrink: 1 }}>
        <Text style={{ fontSize: 17, color: "#ffe0e4", lineHeight: 1.35 }}>
          {message}
        </Text>
      </ScrollView>
      <View style={{ flexDirection: "row", gap: 12 }}>
        <Button
          onPress={() => {
            hideScreen("error");
            endGame();
          }}
        >
          {_("Main menu")}
        </Button>
        <Button onPress={() => native.app.quit()}>{_("Quit")}</Button>
      </View>
    </View>
  ),
  { z: 1000, modal: true },
);

on("error", (error) => {
  const message =
    error instanceof Error ? `${error}\n\n${error.stack ?? ""}` : String(error);
  showScreen("error", { message: message.trimEnd() });
});
