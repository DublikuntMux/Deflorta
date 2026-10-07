import {
  configure,
  screen,
  showScreen,
  View,
  Image,
  TextInput,
} from "deflorta";

configure({ id: "accessibility-test" });
screen(
  "accessibility-test",
  () => (
    <View key="controls">
      <Image
        src="save.png"
        hoverSrc="save-hover.png"
        onPress={() => {}}
        key="save"
        alt="Save game"
      />
      <TextInput
        value=""
        onChangeText={() => {}}
        key="name"
        label="Your name"
        autofocus
      />
    </View>
  ),
  { z: 1000, modal: true },
);
showScreen("accessibility-test");
