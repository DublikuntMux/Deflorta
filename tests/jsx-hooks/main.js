import {
  configure,
  screen,
  showScreen,
  hideScreen,
  View,
  Text,
  TextInput,
  Slider,
  useState,
  native,
  on,
} from "deflorta";
import { setUiHidden } from "deflorta/ui";
import { Counter, probe } from "./counter.jsx";

configure({ id: "jsx-hooks" });

/** @param {{ order: string[], title: string }} props */
function Counters({ order, title }) {
  const [value, setValue] = useState("");
  const [level, setLevel] = useState(0);
  return (
    <View key="counters">
      {order.map((id) => (
        <Counter key={id} id={id} title={title} />
      ))}
      <TextInput key="field" value={value} onChangeText={setValue} />
      <Slider key="slider" value={level} onValueChange={setLevel} />
      <Text key="controlled">
        {value}:{level}
      </Text>
    </View>
  );
}

// A component type replacement must also reset its nested Counter state.
/** @param {{ order: string[], title: string }} props */
function Replacement(props) {
  return <Counters {...props} />;
}

screen("hooks", Counters, { z: 1000 });
showScreen("hooks", { order: ["a", "b"], title: "first" });

on("key", ({ key }) => {
  if (key === "reorder")
    showScreen("hooks", { order: ["b", "a"], title: "first" });
  if (key === "retitle")
    showScreen("hooks", { order: ["b", "a"], title: "second" });
  if (key === "remove") showScreen("hooks", { order: ["b"], title: "second" });
  if (key === "stale") probe.setters.a(999);
  if (key === "hide-ui") setUiHidden(true);
  if (key === "show-ui") setUiHidden(false);
  if (key === "unmount") hideScreen("hooks");
  if (key === "remount") showScreen("hooks", { order: ["a"], title: "third" });
  if (key === "replace") screen("hooks", Replacement, { z: 1000 });
  if (key === "reset") {
    hideScreen("hooks");
    showScreen("hooks", { order: ["a"], title: "reset" });
  }
  if (key === "state") native.audio.voice(JSON.stringify(probe));
});
