// Deflorta demo game. Everything — story, characters and UI — is JavaScript.

import {
  box,
  call,
  character,
  configure,
  defaults,
  dissolve,
  hide,
  hideScreen,
  jump,
  label,
  left,
  menu,
  moveinright,
  music,
  pause,
  randInt,
  right,
  say,
  scene,
  screen,
  show,
  showScreen,
  sound,
  store,
  text,
} from "deflorta";

configure({
  id: "deflorta-demo",
  title: "Deflorta Demo",
  width: 1280,
  height: 720,
  font: "Noto Sans",
});

defaults({ friendship: 0, visitedPark: false });

const eileen = character("Eileen", { color: "#f4b6d2" });
const me = character("Me", { color: "#9fd3ff" });

// A custom screen: a small HUD showing a store variable. Screens are plain
// functions returning elements; re-rendering happens automatically.
screen(
  "friendship",
  () =>
    box(
      {
        key: "hud",
        enter: { dur: 0.3, opacity: 0, y: -10 },
        exit: { dur: 0.3, opacity: 0 },
        style: {
          position: "absolute",
          top: 20,
          left: 20,
          padding: [8, 16],
          radius: 20,
          background: "#00000088",
          flexDirection: "row",
          gap: 8,
        },
      },
      box({ style: { width: 12, height: 12, radius: 6, background: "#f4b6d2", alignSelf: "center" } }),
      text(`Friendship ${store.friendship}`, { style: { color: "#ffffff", fontSize: 18 } }),
    ),
  { z: 5 },
);

label("start", async () => {
  music.play("audio/theme.wav", { fadeIn: 2 });
  scene("bg room", { with: dissolve(1) });
  await say("It's a quiet afternoon. Somebody knocks on the door.");
  sound.play("audio/chime.wav");
  show("eileen happy", { with: moveinright(0.6) });
  await eileen`Hi! I hope I'm not interrupting anything.`;
  showScreen("friendship");

  const answer = await menu("What do you say?", [
    ["Not at all, come in!", "welcome"],
    ["Well... I was busy.", "busy"],
  ]);

  if (answer === "welcome") {
    store.friendship += 2;
    await eileen("Thanks! You always know how to cheer me up.");
  } else {
    store.friendship -= 1;
    show("eileen sad", { with: dissolve(0.3) });
    await eileen("Oh... sorry. I'll be quick, then.");
    show("eileen happy", { with: dissolve(0.3) });
  }

  await eileen("I was going to the park. Want to come along?");
  const goPark = await menu([
    ["Sure, let's go.", true],
    ["Maybe another time.", false],
  ]);
  if (goPark) jump("park");
  jump("home");
});

label("park", async () => {
  store.visitedPark = true;
  scene("bg park", { with: dissolve(1.2) });
  await pause(0.5);
  show("eileen happy", { at: left, with: dissolve(0.5) });
  await eileen("The sky looks amazing at this hour.");
  await call("small_talk");
  show("eileen happy", { at: right });
  await me("We should do this more often.");
  store.friendship += 1;
  jump("ending");
});

label("small_talk", async () => {
  // random() is deterministic across save/load and rollback.
  const birds = randInt(2, 9);
  await eileen(`Look, I can count ${birds} birds on that tree!`);
  await me("You have sharp eyes.");
});

label("home", async () => {
  await eileen("That's fine. Let's just have some tea here.");
  hide("eileen", { with: dissolve(0.5) });
  await say("She disappears into the kitchen, humming a tune.");
  jump("ending");
});

label("ending", async () => {
  hideScreen("friendship");
  const mood = store.friendship >= 3 ? "a wonderful" : store.friendship > 0 ? "a pleasant" : "an awkward";
  await say(`And so ends ${mood} afternoon. (Friendship: ${store.friendship})`);
  music.stop({ fadeOut: 2 });
  scene(null, { with: dissolve(1.5) });
  await pause(1.5);
});
