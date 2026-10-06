import { native, on } from "deflorta/core";
import { box, screen, setUiHidden, showScreen, text, tooltip } from "deflorta/ui";

screen("hover-probe", () => {
  native.audio.voice("render");
  return box({
    key: "hover-probe",
    tooltip: "Saved dialogue",
    onClick: () => native.audio.voice("click"),
  }, text("Save slot"));
});
showScreen("hover-probe");

on("key", (event) => {
  if (event.key === "custom-tooltip") {
    screen("tooltip", () => {
      native.audio.voice("custom-tooltip");
      const value = tooltip();
      return value ? text(value, { key: "custom-tooltip" }) : null;
    }, { z: 950 });
  } else if (event.key === "hide-ui") {
    setUiHidden(true);
  } else if (event.key === "show-ui") {
    setUiHidden(false);
  }
});
