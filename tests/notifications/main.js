import { config, configure, native, notify, on, screen, showScreen } from "deflorta";

configure({ id: "notification-tests", title: "Notifications", autosave: false });
on("boot", () => {
  showScreen("game_menu", { page: "prefs" });
  showScreen("notification-controls");
});

let notice;
let second;
let rejected = 0;

function handle(event) {
  if (!event.down || event.repeat) return;
  switch (event.key) {
    case "create":
      notice = notify("Enabling self-voicing…", { state: "loading" });
      break;
    case "second":
      second = notify("Saved", { duration: null });
      break;
    case "third":
      notify("Download complete", { state: "success", duration: null });
      break;
    case "update":
      notice.update({ message: "Still working…" });
      break;
    case "complete":
      notice.update({ message: "Ready", state: "success" });
      break;
    case "error":
      notice.update({ message: "Could not finish", state: "error", duration: null });
      break;
    case "dismiss":
      notice.dismiss();
      notice.dismiss();
      break;
    case "afterDismiss":
    case "afterExpire":
      notice.update({ message: "Must not return", duration: null });
      break;
    case "dismissSecond":
      second.dismiss();
      break;
    case "expire":
      notice.update({ duration: 0 });
      break;
    case "invalid":
      for (const attempt of [
        () => notify(42),
        () => notify("Invalid", { state: "unknown" }),
        () => notify("Invalid", { duration: -1 }),
        () => notify("Invalid", { duration: Infinity }),
        () => notify("Invalid", { duration: NaN }),
        () => notify("Invalid", { duration: Number.MAX_VALUE }),
        () => notify("Invalid", 4),
        () => notice.update({ message: 42 }),
      ]) {
        try { attempt(); } catch { rejected++; }
      }
      break;
    case "state":
      native.app.configure({ ...config, title: JSON.stringify({ id: notice?.id, secondId: second?.id, rejected }) });
      break;
    default:
      return;
  }
  return true;
}

screen("notification-controls", () => null, {
  z: 2000,
  keys: Object.fromEntries([
    "create", "second", "third", "update", "complete", "error", "dismiss", "afterDismiss",
    "afterExpire", "dismissSecond", "expire", "invalid", "state",
  ].map(key => [key, handle])),
});
