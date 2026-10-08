/** Presentation intervals in milliseconds. No frames means no GPU measurement. */
export function summarize(intervals) {
  const sorted = intervals.filter(ms => Number.isFinite(ms) && ms > 0).sort((a, b) => a - b);
  if (!sorted.length) return null;
  const total = sorted.reduce((sum, ms) => sum + ms, 0);
  const slowest = sorted.slice(-Math.max(1, Math.ceil(sorted.length * 0.01)));
  return {
    frames: sorted.length,
    fps: 1000 * sorted.length / total,
    meanMs: total / sorted.length,
    p95Ms: sorted[Math.ceil(sorted.length * 0.95) - 1],
    p99Ms: sorted[Math.ceil(sorted.length * 0.99) - 1],
    onePercentLowFps: 1000 * slowest.length / slowest.reduce((sum, ms) => sum + ms, 0),
    overBudgetPercent: 100 * sorted.filter(ms => ms > 1000 / 30).length / sorted.length,
  };
}
