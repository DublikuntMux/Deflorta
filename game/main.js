// Deflorta demo game. Everything — story, characters and UI — is JavaScript.

import {
  bob,
  View,
  call,
  character,
  configure,
  defaults,
  dissolve,
  hide,
  hideScreen,
  imageDissolve,
  jump,
  label,
  layeredImage,
  left,
  menu,
  move,
  moveinright,
  music,
  nvlClear,
  nvlNarrator,
  pause,
  persistent,
  pixellate,
  playMovie,
  prompt,
  randInt,
  right,
  say,
  scene,
  screen,
  shake,
  show,
  showScreen,
  sound,
  store,
  Text,
  translations,
  voice,
  wipeleft,
} from "deflorta";

configure({
  id: "deflorta-demo",
  title: "Deflorta Demo",
  version: "2",
  width: 1280,
  height: 720,
  font: "Noto Sans",
  languages: [
    { id: null, name: "English" },
    { id: "uk", name: "Українська" },
  ],
});

// UI strings can be translated inline or in tl/<language>.json.
translations("uk", {
  Start: "Почати",
  Load: "Завантажити",
  Preferences: "Налаштування",
  Quit: "Вийти",
});

defaults({ friendship: 0, name: "Alex" });

// A layered image: the face, outfit and blush change independently.
layeredImage("eileen", [
  { src: "images/eileen/base.png" },
  {
    group: "outfit",
    options: {
      casual: "images/eileen/casual.png",
      formal: "images/eileen/formal.png",
    },
    default: "casual",
  },
  {
    group: "face",
    options: {
      happy: "images/eileen/happy.png",
      sad: "images/eileen/sad.png",
      surprised: "images/eileen/surprised.png",
    },
    default: "happy",
  },
  { attribute: "blush", src: "images/eileen/blush.png" },
]);

const eileen = character("Eileen", { color: "#f4b6d2" });
const me = character("You", { color: "#9fd3ff" });

// A custom screen: a HUD reading the store. Screens re-render automatically.
screen(
  "friendship",
  () => (
    <View
      key="hud"
      enter={{ dur: 0.3, opacity: 0, y: -10 }}
      exit={{ dur: 0.3, opacity: 0 }}
      tooltip="How much Eileen likes you"
      style={{
        position: "absolute",
        top: 20,
        left: 20,
        padding: [8, 16],
        radius: 20,
        background: "#00000088",
        flexDirection: "row",
        gap: 8,
      }}
    >
      <View
        style={{
          width: 12,
          height: 12,
          radius: 6,
          background: "#f4b6d2",
          alignSelf: "center",
        }}
      />
      <Text style={{ color: "#ffffff", fontSize: 18 }}>
        Friendship {store.friendship}
      </Text>
    </View>
  ),
  { z: 5 },
);

label("start", async () => {
  await playMovie("movies/intro.mp4");
  store.name = await prompt("What is your name?", {
    default: "Alex",
    maxLength: 16,
  });
  music.play("audio/theme.wav", { fadeIn: 2 });
  scene("bg room", { with: dissolve(1) });
  await say(
    `It's a quiet afternoon. Somebody knocks on the door, {w=0.4}twice.`,
  );
  sound.play("audio/chime.wav");
  show("eileen happy", { with: moveinright(0.6) });
  voice("audio/voice_hello.wav");
  await eileen`Hi, ${store.name}! I hope I'm not interrupting anything.`;
  showScreen("friendship");
  await eileen(
    "Text can be {b}bold{/b}, {i}italic{/i}, {u}underlined{/u}, {s}struck{/s}, {color=#ffb36b}colored{/color} or {size=+8}bigger{/size}.",
  );
  await eileen(
    "It can pause{w} until you click, carry {ruby=annotations}ruby text{/ruby}, and{p}continue on a new line.",
  );

  const answer = await menu("What do you say?", [
    ["Not at all, come in!", "welcome"],
    ["Well... I was busy.", "busy"],
  ]);

  if (answer === "welcome") {
    store.friendship += 2;
    show("eileen happy blush");
    await eileen("Thanks! You always know how to cheer me up.");
    show("eileen -blush");
  } else {
    store.friendship -= 1;
    show("eileen sad", { with: dissolve(0.3), transform: shake() });
    await eileen("Oh... sorry. I'll be quick, then.");
    show("eileen happy", { transform: null });
  }

  show("eileen", { at: left, with: move(0.8) });
  await eileen("I was going to the park. Want to come along?");
  const goPark = await menu([
    ["Sure, let's go.", true],
    ["Maybe another time.", false],
  ]);
  if (goPark) jump("park");
  jump("home");
});

label("park", async () => {
  scene("bg park", { with: imageDissolve("images/masks/clouds.png", 1.5) });
  await pause(0.5);
  show("eileen happy formal", {
    at: left,
    with: dissolve(0.5),
    transform: bob(8, 2.4),
  });
  await eileen("The sky looks {i}amazing{/i} at this hour.");
  await call("small_talk");
  show("eileen surprised", { at: right, with: move(0.6), transform: null });
  await me("We should do this more often.");
  store.friendship += 1;
  jump("diary");
});

label("small_talk", async () => {
  // random() is deterministic across save/load and rollback.
  const birds = randInt(2, 9);
  await eileen(`Look, I can count ${birds} birds on that tree!`);
  await me("You have sharp eyes.");
});

label("home", async () => {
  await eileen("That's fine. Let's just have some tea here.");
  hide("eileen", { with: wipeleft(0.6) });
  await say("She disappears into the kitchen, humming a tune.{nw}");
  jump("diary");
});

label("diary", async () => {
  hideScreen("friendship");
  scene("bg room", { with: pixellate(1, 40) });
  await nvlNarrator("That evening, I opened my diary.");
  await nvlNarrator("NVL mode shows several lines on one page, like a novel.");
  await nvlNarrator(
    `Today I spent time with Eileen. Friendship: {b}${store.friendship}{/b}.`,
  );
  nvlClear();
  jump("ending");
});

label("ending", async () => {
  persistent.endings = (persistent.endings ?? 0) + 1;
  const mood =
    store.friendship >= 3
      ? "a wonderful"
      : store.friendship > 0
        ? "a pleasant"
        : "an awkward";
  await say(
    `And so ends ${mood} afternoon. You have finished the demo ${persistent.endings} time(s).`,
  );
  music.stop({ fadeOut: 2 });
  scene(null, { with: dissolve(1.5) });
  await pause(1.5);
});
