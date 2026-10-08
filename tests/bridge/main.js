import { configure, native } from "deflorta/core";

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

function rejects(fn, message) {
  try {
    fn();
  } catch (error) {
    assert(String(error).includes(message), `wrong exception: ${error}`);
    return;
  }
  throw new Error(`expected exception: ${message}`);
}

configure({
  id: "bridge-test",
  title: "Привіт 🌸",
  version: { missing: undefined, callback() {}, items: [1, undefined, NaN] },
});

rejects(() => native.connect(null, () => {}), "expects two functions");
rejects(() => native.ui.commit({ children: [{ style: { opacity: "bad" } }] }), "children[0].style.opacity");
rejects(() => native.ui.commit({ get text() { throw new Error("getter failed"); } }), "getter failed");

let tree = Object.create({ text: "inherited text" });
Object.assign(tree, {
  key: "bridge-first",
  style: { opacity: undefined },
  value: { "0": "numeric key", "ключ": "значення", items: [1, undefined, Infinity] },
  onClick: () => native.audio.voice("first"),
  focusable: false, autofocus: true, live: true, modal: true, label: "Accessible tree",
  spans: [{ text: "Rich text", b: true, i: true, u: true, s: true, wait: 0.25, click: true, fast: true,
    get unknown() { throw new Error("unknown span field read"); } }],
  children: [{ t: "text", text: "Привіт 🌸", cps: NaN }],
});
Object.defineProperty(tree, "tooltip", { value: "hidden tooltip", enumerable: false });
Object.defineProperty(tree, "unknown", { enumerable: true, get() { throw new Error("unknown field read"); } });
native.ui.commit(tree, { instant: true, exits: { sprite: { dur: 0.5 }, deleted: null } });
tree = null;
native.ui.commit({ key: "bridge-second", onClick: () => native.audio.voice("second") });

native.connect((event) => {
  if (event.type === "click") {
    assert(!("h" in event), "old numeric handler leaked into event");
    assert(event.button === "left" && event.revealing === false, "click fields");
    if (event.handler === null) native.audio.voice("released");
    else {
      assert(typeof event.handler === "function", "handler is not callable");
      event.handler(event);
      Promise.resolve().then(() => native.audio.voice("microtask"));
    }
  } else if (event.type === "key") {
    assert(event.key === "🌸" && event.down && !event.repeat && event.ctrl && !event.shift && !event.alt, "key fields");
  } else if (event.type === "handler") {
    assert(event.value === 0.1 || Object.is(event.value, -0) || event.value === "Привіт 🌸", "widget value changed");
    event.handler();
  } else if (event.type === "timer") {
    assert(event.id === 4294967297, "timer id truncated");
  }
}, () => native.audio.voice("flush"));
