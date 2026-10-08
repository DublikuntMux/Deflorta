import { clearTimer, configure, invalidate, on, setTimer, storage } from "deflorta";
import { scenes } from "./workloads.js";
import { summarize } from "./stats.mjs";

export const bench = {
  selected: 0, scale: 1, durationMs: 8000, warmupMs: 2000,
  phase: "idle", tick: 0, progress: 0, scratch: {}, live: null,
  results: [],
  status: "Choose a scene, then run it or the full suite.",
  page: "stage",
};
let timer = null;
let active = null;
let queue = [];
let intervals = [];
let phaseStarted = 0;
let lastHud = 0;
let firstSample = true;
let updates = 0;
let sessionStarted = null;

const context = () => ({ scale: bench.scale, tick: bench.tick, scratch: bench.scratch });

function teardown() {
  clearTimer(timer);
  timer = null;
  configure({ frameEvents: false });
  const previous = active;
  active = null;
  previous?.cleanup?.(context());
}

function persist() {
  storage.write("benchmark-latest", {
    version: 1, startedAt: sessionStarted, completedAt: new Date().toISOString(),
    resolution: [1280, 720], warmupMs: bench.warmupMs, measurementMs: bench.durationMs, results: bench.results,
  });
}

function next() {
  if (!queue.length) {
    bench.phase = "idle";
    bench.page = "results";
    bench.status = "Run complete. Results saved to benchmark-latest.json.";
    bench.live = null;
    persist();
    invalidate();
    return;
  }
  bench.selected = queue.shift();
  active = scenes[bench.selected];
  bench.tick = 0;
  bench.scratch = {};
  bench.progress = 0;
  bench.live = null;
  intervals = [];
  updates = 0;
  firstSample = true;
  bench.phase = "warmup";
  bench.status = `${active.name}: warming up`;
  active.setup?.(context());
  configure({ frameEvents: true });
  phaseStarted = Date.now();
  lastHud = phaseStarted;
  invalidate();
  timer = setTimer(16, update);
}

function finish() {
  const result = {
    scene: active.id, name: active.name, scale: bench.scale,
    workload: active.units(bench.scale), durationMs: Date.now() - phaseStarted,
    updates, metrics: summarize(intervals), error: null,
  };
  teardown();
  bench.results.push(result);
  console.log("BENCHMARK_RESULT", result);
  next();
}

function fail(error) {
  if (!active) return;
  const failed = active;
  queue = [];
  try { teardown(); } catch (cleanupError) { console.error(cleanupError); }
  bench.results.push({ scene: failed.id, name: failed.name, scale: bench.scale,
    workload: failed.units(bench.scale), metrics: null, error: String(error) });
  bench.phase = "idle";
  bench.page = "results";
  bench.live = null;
  bench.status = `${failed.name} failed: ${error}`;
  persist();
  invalidate();
}

function update() {
  try {
    if (!active) return;
    const now = Date.now();
    const elapsed = now - phaseStarted;
    const duration = bench.phase === "warmup" ? bench.warmupMs : bench.durationMs;
    if (elapsed >= duration) {
      if (bench.phase === "measure") { finish(); return; }
      bench.phase = "measure";
      phaseStarted = now;
      bench.progress = 0;
      bench.status = `${active.name}: measuring`;
      firstSample = true;
      intervals = [];
      updates = 0;
    } else {
      bench.progress = Math.min(1, Math.max(0, elapsed / duration));
    }
    bench.tick++;
    if (bench.phase === "measure") updates++;
    active.update?.(context());
    if (active.dynamic || now - lastHud >= 250) {
      if (now - lastHud >= 250) {
        bench.live = summarize(intervals.slice(-120));
        lastHud = now;
      }
      invalidate();
    }
    timer = setTimer(16, update);
  } catch (error) { fail(error); }
}

on("frame", ({ frameMs }) => {
  if (bench.phase !== "measure") return;
  // Exclude the first interval straddling the warmup/measurement boundary.
  if (firstSample) { firstSample = false; return; }
  if (frameMs > 0 && Number.isFinite(frameMs)) intervals.push(frameMs);
});
on("error", fail);
on("quit", () => { if (active) cancel(); });
on("boot", () => {
  const saved = storage.read("benchmark-latest");
  if (saved?.version === 1 && Array.isArray(saved.results)) bench.results = saved.results;
  invalidate();
});

export function run(all = false) {
  if (active) return;
  bench.page = "stage";
  bench.results = [];
  sessionStarted = new Date().toISOString();
  queue = all ? scenes.map((_, i) => i) : [bench.selected];
  try { next(); } catch (error) { fail(error); }
}

export function cancel() {
  if (!active) return;
  queue = [];
  teardown();
  bench.phase = "idle";
  bench.live = null;
  bench.status = "Run cancelled. The unfinished scene has no result.";
  // Keep completed suite rows; do not replace a saved run with an empty one.
  if (bench.results.length) persist();
  invalidate();
}

export function select(index) {
  if (active) return;
  bench.selected = index;
  bench.tick = 0;
  bench.scratch = {};
  bench.page = "stage";
  invalidate();
}

export function option(name, value) {
  if (active) return;
  bench[name] = value;
  invalidate();
}
