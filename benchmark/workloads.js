import {
  atl, Grid, Image, music, RichText, sound, storage, Text, Video, View,
  wipeleft, pixellate, dissolve,
} from "deflorta";

export const colors = ["#48bdd4", "#f2b66a", "#9383dc", "#82caa5"];
const sprites = ["images/orb-0.png", "images/orb-1.png", "images/orb-2.png", "images/orb-3.png"];
const fill = { position: "absolute", left: 0, top: 0, width: "100%", height: "100%" };
const each = (count, fn) => Array.from({ length: count }, (_, i) => fn(i));
const quantity = (base, scale) => Math.round(base * scale);
const place = i => ({ left: (i * 73) % 780, top: (i * 47) % 420 });

function Orbs({ count, animated = false }) {
  return each(count, i => <Image key={`orb-${i}`} src={sprites[i % 4]} alt=""
    style={{ position: "absolute", ...place(i), width: 36, height: 36 }}
    transform={animated ? atl().linear(0.7 + i % 7 / 10, { x: 24, y: -18, rotate: 180 })
      .linear(0.7 + i % 7 / 10, { x: 0, y: 0, rotate: 360 }).repeat() : undefined} />);
}

function Layers({ count }) {
  return each(count, i => <View key={`layer-${i}`} style={{
    ...fill, left: i % 5 * 3, top: i % 7 * 2, width: "94%", height: "94%",
    background: `${colors[i % 4]}12`, radius: 48, borderWidth: 1, borderColor: `${colors[i % 4]}44`,
  }} />);
}

function Paragraphs({ count, tick }) {
  return <Grid columns={6} style={{ ...fill, gap: 5, overflow: "hidden" }}>
    {each(count, i => <RichText key={`text-${i}`} style={{ fontSize: 12, padding: 4, color: "#dce8f4" }}>
      {`{b}Signal ${i}{/b} {color=${colors[i % 4]}}${tick % 1000}{/color}\n` +
        "{i}Shaping{/i} café, naïve, Привіт. {u}Glyph cache{/u} 0123456789."}
    </RichText>)}
  </Grid>;
}

function Tiles({ count, tick }) {
  return <Grid columns={8} style={{ ...fill, gap: 4, overflow: "hidden" }}>
    {each(count, i => <View key={`tile-${i}`} style={{
      padding: 4 + (tick + i) % 3, gap: 2, flexDirection: "column", background: "#263c54",
      borderWidth: 1, borderColor: "#355573", radius: 4,
    }}>
      <View style={{ flexDirection: "row", gap: 3 }}>
        <View style={{ width: `${20 + (tick + i) % 60}%`, height: 5, background: colors[i % 4] }} />
        <View style={{ flexGrow: 1, height: 5, background: "#4c6380" }} />
      </View>
      <View style={{ flexDirection: "row", justifyContent: "space-between" }}>
        <Text style={{ fontSize: 10 }}>{i}</Text>
        <View style={{ width: 8, height: 8, radius: 4, background: colors[(i + 1) % 4] }} />
      </View>
    </View>)}
  </Grid>;
}

function Transitions({ count, tick }) {
  const phase = Math.floor(tick / 30);
  const transition = [dissolve(0.4), wipeleft(0.4), pixellate(0.4, 24)][phase % 3];
  return each(count, i => <Image key={`transition-${phase}-${i}`} src={sprites[(i + phase) % 4]}
    alt="" enter={transition.in ? { dur: transition.dur, ...transition.in } : undefined}
    exit={transition.out ? { dur: transition.dur, ...transition.out } : undefined}
    style={{ position: "absolute", ...place(i), width: 64, height: 64 }} />);
}

function Media({ count }) {
  return <Grid columns={count > 1 ? 2 : 1} style={{ ...fill, gap: 8 }}>
    {each(count, i => <Video key={`video-${i}`} src="movies/signal.webm" loop fit="contain"
      style={{ width: "100%", height: count > 2 ? 215 : 440 }} />)}
  </Grid>;
}

function Scope({ tick, title, detail }) {
  return <View style={{ ...fill, padding: 32, justifyContent: "center", gap: 24, flexDirection: "column" }}>
    <Text style={{ fontSize: 32, color: "#dce8f4" }}>{title}</Text>
    <View style={{ flexDirection: "row", height: 120, alignItems: "center", gap: 4 }}>
      {each(72, i => <View key={`bar-${i}`} style={{ width: 6,
        height: 12 + Math.abs(Math.sin((tick + i) * 0.18)) * 100,
        background: colors[i % 4], radius: 3 }} />)}
    </View>
    <Text style={{ fontSize: 16, color: "#a9bed1" }}>{detail}</Text>
  </View>;
}

// Scene metadata and work stay together so the displayed workload is inspectable.
export const scenes = [
  { id: "baseline", name: "Baseline", aspect: "Presentation overhead", units: () => "Empty stage",
    render: () => <View style={{ ...fill, justifyContent: "center", alignItems: "center", gap: 12, flexDirection: "column" }}>
      <Text style={{ fontSize: 44, color: "#48bdd4" }}>Deflorta / Bench</Text>
      <Text style={{ fontSize: 18, color: "#a9bed1" }}>A reference for every scene that follows.</Text>
    </View> },
  { id: "sprites", name: "Sprite field", aspect: "Texture reuse and quad batching",
    units: s => `${quantity(256, s)} sprites / 4 textures`,
    render: ({ scale }) => <Orbs count={quantity(256, scale)} /> },
  { id: "overdraw", name: "Glass stack", aspect: "Alpha blending and fill rate",
    units: s => `${quantity(24, s)} overlapping panels`,
    render: ({ scale }) => <Layers count={quantity(24, scale)} /> },
  { id: "text", name: "Typesetter", aspect: "Rich text, shaping and glyph uploads", dynamic: true,
    units: s => `${quantity(72, s)} changing paragraphs`,
    render: ({ scale, tick }) => <Paragraphs count={quantity(72, scale)} tick={tick} /> },
  { id: "layout", name: "Layout loom", aspect: "Nested grid and flex invalidation", dynamic: true,
    units: s => `${quantity(96, s)} changing nested tiles`,
    render: ({ scale, tick }) => <Tiles count={quantity(96, scale)} tick={tick} /> },
  { id: "animation", name: "Orbit machine", aspect: "Native transforms and rotation",
    units: s => `${quantity(192, s)} looping transforms`,
    render: ({ scale }) => <Orbs count={quantity(192, scale)} animated /> },
  { id: "transitions", name: "Scene change", aspect: "Enter/exit lifetimes and reveal masks", dynamic: true,
    units: s => `${quantity(48, s)} replacements every 30 updates`,
    render: ({ scale, tick }) => <Transitions count={quantity(48, scale)} tick={tick} /> },
  { id: "script", name: "Script furnace", aspect: "JavaScript CPU and UI commits", dynamic: true,
    units: s => `${quantity(20000, s)} operations per update`,
    update: ({ scale, tick, scratch }) => {
      let value = scratch.checksum ?? 1;
      for (let i = 0; i < quantity(20000, scale); i++) value = Math.imul(value ^ (i + tick), 1664525) + 1013904223 | 0;
      scratch.checksum = value;
    },
    render: ({ tick, scratch }) => <Scope tick={tick} title={`Checksum ${scratch.checksum ?? 1}`}
      detail="Integer arithmetic drives a changing UI tree." /> },
  { id: "audio", name: "Sound desk", aspect: "Music decoding and concurrent effects", dynamic: true,
    units: s => `Looping music + ${quantity(4, s)} effects every 15 updates`,
    setup: () => music.play("audio/pulse.wav", { volume: 0.08, fadeIn: 0 }),
    update: ({ scale, tick }) => {
      if (tick % 15 === 0) each(quantity(4, scale), () => sound.play("audio/tick.wav", { volume: 0.03 }));
    },
    cleanup: () => music.stop({ fadeOut: 0 }),
    render: ({ tick }) => <Scope tick={tick} title="Music + effects"
      detail="Use an active audio device. Listen for clicks, gaps, or distortion." /> },
  { id: "video", name: "Video wall", aspect: "VP9 decoding and frame uploads",
    units: s => `${quantity(1, s)} looping 640 × 360 / 30 FPS players`,
    render: ({ scale }) => <Media count={quantity(1, scale)} /> },
  { id: "storage", name: "Archive desk", aspect: "JSON serialization and queued persistence", dynamic: true,
    units: s => `${quantity(2, s)} × 8 KiB queued every 6 updates`,
    setup: ({ scratch }) => { scratch.payload = "benchmark-data:".padEnd(8192, "0123456789"); },
    update: ({ scale, tick, scratch }) => {
      if (tick % 6 !== 0) return;
      each(quantity(2, scale), i => {
        const name = `bench-scratch-${i}`;
        storage.write(name, { tick, payload: scratch.payload });
        if (storage.read(name)?.tick !== tick) throw new Error("Storage snapshot did not reflect a queued write");
      });
    },
    cleanup: ({ scale }) => each(quantity(2, scale), i => storage.remove(`bench-scratch-${i}`)),
    render: ({ tick }) => <Scope tick={tick} title="Serialize / queue / read"
      detail="Measures game responsiveness while writes queue; disk completion is asynchronous." /> },
  { id: "combined", name: "All systems", aspect: "Mixed visual novel workload", dynamic: true,
    units: s => `${quantity(96, s)} sprites + text, layout, blending and video`,
    render: ({ scale, tick }) => <>
      <Layers count={quantity(8, scale)} />
      <Orbs count={quantity(96, scale)} animated />
      <View style={{ position: "absolute", left: 24, top: 24, width: 360, height: 180 }}>
        <Video src="movies/signal.webm" loop fit="contain" style={fill} />
      </View>
      <View style={{ position: "absolute", left: 420, top: 24, width: 360, height: 180 }}>
        <Tiles count={quantity(16, scale)} tick={tick} />
      </View>
      <View style={{ position: "absolute", left: 24, top: 240, width: 760, height: 190 }}>
        <Paragraphs count={quantity(12, scale)} tick={tick} />
      </View>
    </> },
];
