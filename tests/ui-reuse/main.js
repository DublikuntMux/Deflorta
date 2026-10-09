import {
  configure, invalidate, native, on, screen, showScreen, Text, useMemo, useState, View,
} from "deflorta";

configure({ id: "ui-reuse" });

const state = { label: "first", renders: 0 };
// Mutated in place: signatures must still notice.
const shared = { color: "#ffffff" };
let bump = () => {};

function Counted() {
  const [count, setCount] = useState(0);
  bump = () => setCount((value) => value + 1);
  state.renders++;
  return <View key="memo">
    <Text key="count">{`count ${count}`}</Text>
    <Text key="global">{state.label}</Text>
  </View>;
}

function Main() {
  const memo = useMemo(() => <Counted />, []);
  return <View key="main">
    <View key="static"><Text key="plain">static</Text></View>
    <Text key="label">{state.label}</Text>
    <View key="button" onPress={() => {}}><Text key="caption">press</Text></View>
    <Text key="shared" style={shared}>shared</Text>
    {memo}
  </View>;
}

screen("main", Main);
showScreen("main");

on("key", ({ key }) => {
  if (key === "label") state.label = "second";
  if (key === "mutate") shared.color = "#ff0000";
  if (key === "bump") bump();
  if (key === "direct") native.ui.commit({ key: "elsewhere" });
  if (key === "renders") native.audio.voice(String(state.renders));
  invalidate();
});
