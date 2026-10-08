# Deflorta / Bench

A standalone benchmark game with twelve selectable engine workloads. The demo
in `game/` is separate. All textures and media are included, with a bundled Noto
Sans font so the same fixtures run on each machine.

From the repository root, rebuild the distribution after changing the engine:

```sh
python3 scripts/build-dist.py
dist/deflorta check benchmark
dist/deflorta run benchmark
```

Choose a scene and a load multiplier (0.5×, 1×, 2×, or 4×), then **Run scene**
or **Run suite**. Every scene warms up for two seconds, then measures for the
selected 1, 8, or 20 seconds. A default full suite takes about two minutes.
S runs a scene; A runs the suite; Escape cancels; R toggles results.
N/P select the next/previous scene. Arrow keys navigate controls and Enter
activates the focused control. Load and duration cannot change during a run.

For comparable measurements, use the same runtime build, window size, load,
duration, display refresh rate, preferences, and audio device. Keep the window
visible and focused. Hide the debug console and inspectors with F12, and turn
off self-voicing with F6 if enabled. VSync can cap FPS at the display refresh
rate. Use 8 or 20 seconds for comparisons; 1 second is useful for smoke tests.
Measurements include the common benchmark interface and frame-event overhead.
Use the baseline to understand that overhead. Small differences between runs
are expected; repeat a run before drawing conclusions.

## Workloads at 1×

| Scene | Workload | Main aspect |
| --- | --- | --- |
| Baseline | Empty reference stage | Presentation overhead |
| Sprite field | 256 sprites sharing four textures | Texture reuse and batching |
| Glass stack | 24 almost-full-stage translucent panels | Blending and fill rate |
| Typesetter | 72 changing rich-text paragraphs | Shaping, markup, glyph uploads |
| Layout loom | 96 nested tiles with changing sizes | Grid/flex layout invalidation |
| Orbit machine | 192 looping native transforms | Animation and rotation |
| Scene change | 48 replacements every 30 updates | Dissolve, wipe, pixel masks, exit cleanup |
| Script furnace | 20,000 integer operations per update | JavaScript CPU and UI commits |
| Sound desk | Looping music and four simultaneous effects every 15 updates | Audio decoding and mixing |
| Video wall | One looping 640×360 VP9 player at 30 FPS | Decoding and frame uploads |
| Archive desk | Two 8 KiB JSON writes every six updates | Serialization and persistence queue |
| All systems | Sprites, text, layout, blending, video, animation | Mixed workload |

Updates request a 16 ms timer. Under load their actual cadence may be slower;
the report records the number of measurement updates. Counts scale with load,
except the baseline. The 0.5× video workload rounds up to one player. The combined
scene keeps one video player while scaling its other visual workloads.
Audio needs an active device; headless tests have no audio device. An audio run
measures presentation responsiveness under the audio workload, not audio latency
or underrun counts. Listen for audible problems. Storage writes queue on a
worker; the scene validates immediate snapshot reads and measures responsiveness,
not durable-write latency. It reuses a bounded set of keys and removes them on
completion or cancellation. Cross-scene caches may remain resident during a
suite, so these are warm-runtime workloads rather than cold-start tests.

## Results

Results show average FPS, 1% low FPS, p95/p99 presentation intervals, and the
percentage of intervals over 33.33 ms. Lower frame times and fewer long frames
are better. Average FPS is `1000 / mean interval`; 1% low FPS is `1000 / mean
of the slowest ceil(1% × sample count) intervals`. Percentiles use nearest rank.
Warmup and the first interval crossing into measurement are excluded.

The native `frame` event measures intervals between successful window
presentations using Rust's monotonic clock. Skipped surface acquisitions do
not count. This is presentation cadence, including CPU work and VSync, rather
than a GPU timestamp query. Frame events and continuous redraws are enabled
only while a scene runs. Run duration uses the scripting wall clock.

Each completed scene is printed to the console as `BENCHMARK_RESULT`. At the
end of a run, the full JSON report is saved through the engine's storage API as
`benchmark-latest.json`, under the per-game data directory. On Linux this is
`~/.local/share/deflorta/deflorta-benchmark/benchmark-latest.json` (or under
`$XDG_DATA_HOME`). The report contains settings, workload descriptions, sample
counts, measured durations, update counts, metrics, and errors. Results reload
on the next launch. A cancelled scene is omitted; completed suite rows are kept.

## Verification and fixtures

```sh
node --test tests/benchmark/stats.test.mjs
dist/deflorta run benchmark --test tests/benchmark/smoke.json
dist/deflorta run benchmark --test tests/benchmark/screens.json
cargo test -p deflorta-engine-core --features dev-console presentation_events_are_opt_in_monotonic_and_reset_on_disable
```

The smoke script runs all twelve scenes at 1× and one-second measurement windows,
captures the menu and results, and exercises cancellation. Use a fresh
`XDG_DATA_HOME` to isolate stored reports. Headless mode simulates updates and
draws screenshots but does not present window frames; its metrics are explicitly
null. Use a visible window for performance comparisons.
The screen script captures each stress scene during warmup and cancels it,
exercising scene selection, keyboard shortcuts, rendering, and cleanup.
