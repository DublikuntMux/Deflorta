import { configure, native, on, readText, setTimer, storage } from "deflorta";

configure({ id: "headless-regressions", autosave: false });
let quitCount = 0;
on("quit", () => storage.write("quit-count", ++quitCount));
on("boot", () => {
  setTimer(300, () => storage.write("after-quit", true));
  const mode = readText("mode.txt");
  if (mode === "boot") native.app.quit();
  if (mode === "wait") setTimer(50, () => native.app.quit());
});
on("key", event => {
  if (event.down && event.key === "q") native.app.quit();
});
