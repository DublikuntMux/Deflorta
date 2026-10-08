import { configure, on, storage } from "deflorta/core";
import { screen, showScreen } from "deflorta/ui";

configure({ id: "storage-worker", autosave: false });

function showState(extra = {}) {
  configure({ title: JSON.stringify({ value: storage.read("value"), ...extra }) });
}

on("boot", () => {
  storage.write("transient", { ready: true });
  if (!storage.read("transient")?.ready ||
      !storage.list().some((entry) => entry.name === "transient")) {
    throw new Error("queued writes must be readable and listed");
  }
  if (!storage.remove("transient") || storage.remove("transient") ||
      storage.read("transient") !== null ||
      storage.list().some((entry) => entry.name === "transient")) {
    throw new Error("queued deletion must update the snapshot");
  }
  showState();
});

screen("tooltip", () => null, {
  z: 1000000,
  keys: { write() {
    storage.write("value", { version: "new" });
    showState();
  } },
});
showScreen("tooltip");

on("error", (error) => showState({ error: error.message }));
