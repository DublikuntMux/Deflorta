import {
  configure, label, native, Pressable, screen, ScrollView, Text, useMemo, View,
} from "deflorta";
import { bench, cancel, option, run, select } from "./controller.js";
import { scenes } from "./workloads.js";

configure({
  id: "deflorta-benchmark", title: "Deflorta / Bench", version: "1",
  width: 1280, height: 720, font: "Noto Sans", clearColor: "#15283b",
  autosave: false, textSpeed: 0,
});

const palette = { ink: "#e4eef6", muted: "#a9bed1", line: "#35516c", accent: "#48bdd4" };
const positioned = (left, top, width, height) => ({ position: "absolute", left, top, width, height });
const running = () => bench.phase !== "idle";
const number = value => value == null ? "—" : value.toFixed(1);

function Button({ children, onPress, disabled = false, selected = false, style }) {
  return <Pressable onPress={onPress} disabled={disabled} style={{
    padding: [7, 12], radius: 5, borderWidth: 1,
    borderColor: selected ? palette.accent : palette.line,
    background: selected ? "#285773" : "#223c54", ...style,
  }} hover={{ background: "#35647b", borderColor: palette.accent }}>
    <Text style={{ fontSize: 15, color: disabled ? "#70879a" : palette.ink }}>{children}</Text>
  </Pressable>;
}

function Results() {
  return <View style={{ ...positioned(0, 0, 824, 456), padding: 20, gap: 12, flexDirection: "column" }}>
    <Text style={{ fontSize: 24 }}>Run results</Text>
    <Text style={{ fontSize: 13, color: palette.muted }}>
      Presentation FPS and interval percentiles. Lower frame times are better.
    </Text>
    <View style={{ flexDirection: "row", padding: [8, 4], background: "#29435a" }}>
      {["Scene", "FPS", "1% low", "p95 ms", "p99 ms", ">33 ms"].map((title, i) =>
        <Text key={title} style={{ width: i === 0 ? 270 : 95, fontSize: 14 }}>{title}</Text>)}
    </View>
    <ScrollView style={{ height: 290, gap: 3 }}>
      {!bench.results.length && <Text style={{ color: palette.muted }}>
        Run a scene to measure it. Run the suite to compare every workload.
      </Text>}
      {bench.results.map((result, i) => <View key={`${result.scene}-${i}`} style={{
        flexDirection: "row", padding: [5, 4], background: i % 2 ? "#233b51" : "#1b3146",
      }}>
        <Text style={{ width: 270, fontSize: 14 }}>{result.name} / {result.scale}×</Text>
        {result.error ? <Text style={{ fontSize: 13, color: "#f2b66a" }}>{result.error}</Text>
          : !result.metrics ? <Text style={{ fontSize: 13, color: palette.muted }}>No presented-frame samples</Text>
          : [result.metrics.fps, result.metrics.onePercentLowFps, result.metrics.p95Ms,
            result.metrics.p99Ms, result.metrics.overBudgetPercent].map((value, j) =>
            <Text key={j} style={{ width: 95, fontSize: 14 }}>{number(value)}{j === 4 ? "%" : ""}</Text>)}
      </View>)}
    </ScrollView>
  </View>;
}

function Lab() {
  const scene = scenes[bench.selected];
  const content = useMemo(() => scene.render({ scale: bench.scale, tick: bench.tick, scratch: bench.scratch }),
    [bench.selected, bench.scale, bench.scratch, scene.dynamic ? bench.tick : 0]);
  return <View style={{ position: "absolute", left: 0, top: 0, right: 0, bottom: 0,
    background: "#15283b", color: palette.ink, fontSize: 16 }}>
    <View style={{ ...positioned(28, 24, 1224, 90), flexDirection: "row", alignItems: "center", gap: 32 }}>
      <View style={{ width: 360, gap: 5, flexDirection: "column" }}>
        <Text style={{ fontSize: 34, fontWeight: 700 }}>Deflorta / Bench</Text>
        <Text style={{ fontSize: 15, color: palette.muted }}>Twelve scenes. One engine. Real workloads.</Text>
      </View>
      <View style={{ gap: 5, flexDirection: "column" }}>
        <Text style={{ fontSize: 13, color: palette.muted }}>Load</Text>
        <View style={{ flexDirection: "row", gap: 5 }}>
          {[0.5, 1, 2, 4].map(scale => <Button key={scale} disabled={running()}
            selected={bench.scale === scale} onPress={() => option("scale", scale)}>{scale}×</Button>)}
        </View>
      </View>
      <View style={{ gap: 5, flexDirection: "column" }}>
        <Text style={{ fontSize: 13, color: palette.muted }}>Measure / scene</Text>
        <View style={{ flexDirection: "row", gap: 5 }}>
          {[1, 8, 20].map(seconds => <Button key={seconds} disabled={running()}
            selected={bench.durationMs === seconds * 1000}
            onPress={() => option("durationMs", seconds * 1000)}>{seconds}s</Button>)}
        </View>
      </View>
      <Button disabled={running()} selected={bench.page === "results"}
        onPress={() => option("page", bench.page === "results" ? "stage" : "results")}>Results</Button>
      <Button onPress={() => native.app.quit()}>Quit</Button>
    </View>

    <View style={{ ...positioned(28, 138, 350, 466), gap: 3, flexDirection: "column" }}>
      <Text style={{ fontSize: 15, color: palette.muted, margin: [0, 0, 8, 0] }}>Choose a workload</Text>
      {scenes.map((item, index) => <Pressable key={item.id} disabled={running()}
        onPress={() => select(index)} style={{ flexDirection: "row", justifyContent: "space-between",
          padding: [5, 12], radius: 4, borderWidth: 1,
          borderColor: index === bench.selected ? palette.accent : "#35516c66",
          background: index === bench.selected ? "#285773" : "#1e3449",
        }} hover={{ background: "#335773", borderColor: palette.accent }}>
        <Text style={{ fontSize: 15 }}>{item.name}</Text>
        <Text style={{ fontSize: 13, color: palette.muted }}>{item.id}</Text>
      </Pressable>)}
    </View>

    <View style={{ ...positioned(404, 132, 824, 28), flexDirection: "row", justifyContent: "space-between" }}>
      <Text style={{ fontSize: 18, fontWeight: 700 }}>{scene.name}</Text>
      <Text style={{ fontSize: 14, color: palette.muted }}>{scene.units(bench.scale)}</Text>
    </View>
    <View key="stage" style={{ ...positioned(404, 168, 824, 456), background: "#1b3146",
      borderWidth: 1, borderColor: palette.line, radius: 8, overflow: "hidden" }}>
      {bench.page === "results" ? <Results /> : <View key={`workload-${scene.id}`}
        style={{ position: "absolute", left: 0, top: 0, width: "100%", height: "100%" }}>{content}</View>}
    </View>

    <View style={{ ...positioned(28, 632, 350, 60), gap: 8, flexDirection: "column" }}>
      <View style={{ flexDirection: "row", gap: 8 }}>
        <Button disabled={running()} onPress={() => run()} style={{ flexGrow: 1 }}>Run scene</Button>
        <Button disabled={running()} onPress={() => run(true)} style={{ flexGrow: 1 }}>Run suite</Button>
        <Button disabled={!running()} onPress={cancel}>Stop</Button>
      </View>
      <Text style={{ fontSize: 12, color: palette.muted }}>S: scene   A: suite   Esc: stop   R: results</Text>
    </View>
    <View style={{ ...positioned(404, 636, 824, 60), gap: 7, flexDirection: "column" }}>
      <View style={{ flexDirection: "row", justifyContent: "space-between" }}>
        <Text style={{ fontSize: 14, color: palette.muted }}>{running() ? bench.status : scene.aspect}</Text>
        <Text style={{ fontSize: 14, color: palette.accent }}>
          {running() ? bench.live ? `${number(bench.live.fps)} FPS / ${number(bench.live.p95Ms)} ms p95` : "Waiting for frame samples"
            : "1280 × 720 / 2s warmup"}
        </Text>
      </View>
      <View style={{ height: 4, background: "#35516c", radius: 2 }}>
        <View style={{ width: `${running() ? Math.round(bench.progress * 100) : 0}%`, height: 4,
          background: bench.phase === "warmup" ? "#f2b66a" : palette.accent }} />
      </View>
      <Text style={{ fontSize: 12, color: palette.muted }}>{running() ? "Warmup is excluded. Keep this window visible and focused."
        : bench.status}</Text>
    </View>
  </View>;
}

screen("main_menu", Lab, { modal: true, keys: {
  s: () => run(), S: () => run(), a: () => run(true), A: () => run(true), Escape: cancel,
  r: () => option("page", bench.page === "results" ? "stage" : "results"),
  n: () => select((bench.selected + 1) % scenes.length),
  p: () => select((bench.selected + scenes.length - 1) % scenes.length),
  "1": () => option("durationMs", 1000),
} });
// Keep a valid start label for tooling; the lab is the main menu itself.
label("start", () => {});
