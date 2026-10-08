import assert from "node:assert/strict";
import { test } from "node:test";
import { summarize } from "../../benchmark/stats.mjs";

test("missing or invalid presentation samples do not produce fabricated FPS", () => {
  assert.equal(summarize([]), null);
  assert.equal(summarize([0, -1, NaN, Infinity]), null);
});

test("steady 60 FPS cadence reports matching percentiles and one percent low", () => {
  const result = summarize(Array(120).fill(1000 / 60));
  assert.equal(result.frames, 120);
  assert.ok(Math.abs(result.fps - 60) < 1e-10);
  assert.equal(result.p95Ms, 1000 / 60);
  assert.ok(Math.abs(result.onePercentLowFps - 60) < 1e-10);
  assert.equal(result.overBudgetPercent, 0);
});

test("stalls affect weighted FPS, percentiles, and slowest-one-percent mean", () => {
  const samples = [...Array(98).fill(10), 40, 80];
  const result = summarize(samples);
  assert.equal(result.fps, 100000 / 1100);
  assert.equal(result.meanMs, 11);
  assert.equal(result.p95Ms, 10);
  assert.equal(result.p99Ms, 40);
  assert.equal(result.onePercentLowFps, 12.5);
  assert.equal(result.overBudgetPercent, 2);
  assert.deepEqual(samples.slice(-2), [40, 80]);
});

test("one-percent low averages all tail samples using nearest-rank selection", () => {
  const result = summarize([...Array(198).fill(10), 40, 80]);
  assert.equal(result.onePercentLowFps, 1000 / 60);
  assert.equal(result.p99Ms, 10);
});
