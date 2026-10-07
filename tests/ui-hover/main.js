import { native, on } from "deflorta/core";
import {
  View,
  Text,
  screen,
  setUiHidden,
  showScreen,
  tooltip,
} from "deflorta/ui";

screen("hover-probe", () => {
  native.audio.voice("render");
  return (
    <View
      key="hover-probe"
      tooltip="Saved dialogue"
      onPress={() => native.audio.voice("click")}
    >
      <Text>Save slot</Text>
    </View>
  );
});
showScreen("hover-probe");

on("key", (event) => {
  if (event.key === "custom-tooltip") {
    screen(
      "tooltip",
      () => {
        native.audio.voice("custom-tooltip");
        const value = tooltip();
        return value ? <Text key="custom-tooltip">{value}</Text> : null;
      },
      { z: 950 },
    );
  } else if (event.key === "hide-ui") {
    setUiHidden(true);
  } else if (event.key === "show-ui") {
    setUiHidden(false);
  }
});
