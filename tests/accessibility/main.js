import { configure, screen, showScreen, box, imageButton, input } from "deflorta";

configure({ id: "accessibility-test" });
screen("accessibility-test", () => box(
  { key: "controls" },
  imageButton("save.png", "save-hover.png", () => {}, { key: "save", alt: "Save game" }),
  input("", () => {}, { key: "name", label: "Your name", autofocus: true }),
), { z: 1000, modal: true });
showScreen("accessibility-test");
