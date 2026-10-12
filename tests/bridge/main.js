import { configure, native, storage } from "deflorta/core";

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

for (const api of [native, ...Object.values(native).filter(value => typeof value === "object"), storage]) {
  assert(Object.isFrozen(api), "API object is mutable");
  assert(!Reflect.set(api, "added", true), "API accepts new properties");
  for (const key of Object.keys(api)) {
    const original = api[key];
    assert(!Reflect.set(api, key, () => {}), `API allows replacing ${key}`);
    assert(!Reflect.deleteProperty(api, key), `API allows deleting ${key}`);
    assert(api[key] === original, `API changed ${key}`);
  }
}
try {
  native.audio.voice = () => {};
  throw new Error("native method assignment succeeded");
} catch (error) {
  assert(error instanceof TypeError, "native method assignment must throw TypeError");
}

configure({
  id: "bridge-test",
  title: "Привіт 🌸",
  version: { missing: undefined, callback() {}, items: [1, undefined, NaN] },
});

rejects(() => native.connect(null, () => {}), "expects two functions");
rejects(() => native.ui.commit({ children: [{ style: { opacity: "bad" } }] }), "children[0].style.opacity");
rejects(() => native.ui.commit({ get text() { throw new Error("getter failed"); } }), "getter failed");
rejects(() => native.ui.commit({ style: { padding: [8, 12] } }), "style.padding");
rejects(() => native.ui.commit({ style: { margin: [1, 2, 3, 4] } }), "style.margin");
rejects(() => native.ui.commit({ anchor: [0.5, 1] }), "anchor");
rejects(() => native.ui.commit({ transform: { steps: [{ set: { crop: [0, 0, 1, 1] } }] } }), "transform");

let tree = Object.create({ text: "inherited text" });
Object.assign(tree, {
  key: "bridge-first",
  style: {
    opacity: undefined,
    padding: { top: 8, right: 12, bottom: 8, left: 12 },
    margin: { bottom: -4 },
  },
  anchor: { x: 0.5, y: 1 },
  transform: { steps: [{ set: { crop: { x: 0.1, y: 0.2, width: 0.6, height: 0.7 } } }] },
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
