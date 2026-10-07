// The entry point of the game. Everything — story, characters and screens —
// is JavaScript; `deflorta.d.ts` gives editors completion and type checks.

import {
  character,
  configure,
  defaults,
  dissolve,
  jump,
  label,
  menu,
  say,
  scene,
  store,
  Pressable,
  Text,
  screen,
  showScreen,
  useState,
} from "deflorta";

configure({
  id: __ID__,
  title: __TITLE__,
  version: "1",
  width: 1280,
  height: 720,
  font: "Noto Sans",
});

defaults({ visits: 0 });

const guide = character("Guide", { color: "#f4b6d2" });

// Hook state belongs to the UI; saved story variables belong in store.
function Visits() {
  const [expanded, setExpanded] = useState(false);
  return (
    <Pressable
      onPress={() => setExpanded((value) => !value)}
      style={{
        position: "absolute",
        top: 20,
        right: 20,
        padding: [8, 16],
      }}
    >
      <Text>{expanded ? `Visits: ${store.visits}` : "Show visits"}</Text>
    </Pressable>
  );
}
screen("visits", Visits, { z: 5 });

label("start", async () => {
  // Backgrounds come from images/<name>.png; scene() without a name clears the screen.
  scene(null, { with: dissolve(1) });
  await say("Welcome to your new visual novel.");
  await guide("Edit {b}main.js{/b} to write your story.");

  const answer = await menu("Where to next?", [
    ["Tell me more", "more"],
    ["That's enough for now", "done"],
  ]);
  if (answer === "more") jump("more");
  await guide("See you soon!");
});

label("more", async () => {
  store.visits += 1;
  showScreen("visits");
  await guide(
    "Run {i}deflorta check{/i} to find mistakes, and {i}deflorta publish{/i} to ship the game.",
  );
  await guide(
    "Translations live in tl/. Create one with {i}deflorta translate update uk{/i}.",
  );
});
