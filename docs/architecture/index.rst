Engine architecture
===================

Deflorta has a native Rust core and an embedded JavaScript runtime executed
by SpiderMonkey. JavaScript describes the story and everything the player
sees: characters, menus, save screens, and preferences. Rust handles layout,
animation, text shaping, input, media, accessibility, and drawing.

Design goals
------------

* **Cross-platform:** Rust with winit for windows/input, wgpu for graphics,
  kira/cpal for audio, and SpiderMonkey through ``mozjs``. Android has a
  dedicated entry point and Kotlin activity template.
* **Event-driven performance:** JavaScript runs on events and timers;
  native animation and reveal work drive frames when needed.
* **Portable distributions:** a launcher beside ``game.dm`` and any required
  runtime resources. Replay-based saves avoid serializing JavaScript stacks.
* **Customizable interfaces:** the story runtime and default screens are
  embedded JavaScript modules; games replace default screens through the
  public screen API.

Web builds, 3D rendering, and Live2D(more likely be own open format) are outside the current scope.

Subsystems
----------

The engine is split into domain crates. Dependencies flow from the platform
front end and headless runner through engine coordination to UI, the JS bridge,
and assets. UI and the JS bridge share descriptions and callback IDs from
``deflorta-common``; UI never depends on SpiderMonkey. This lets Cargo compile
UI and scripting in parallel and keeps changes within their owning domains.

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Crate / path
     - Responsibility
   * - ``crates/js-bridge``
     - SpiderMonkey host, modules, microtasks, typed native calls, and direct
       JavaScript values, persistence workers, and embedded runtime modules.
   * - ``crates/engine-core/src/engine.rs``
     - Route input into JavaScript, apply commands, own timers/UI/audio,
       gate commits on image decoding, and capture save thumbnails.
   * - ``crates/ui``
     - Retained-tree reconciliation, taffy layout, animation programs,
       typewriter, rich text, scrolling, focus, widgets, and accessibility.
   * - ``crates/engine-core/src/render/``
     - wgpu quads, glyphon text, and offscreen capture.
   * - ``crates/assets/src/video/`` and ``video.rs``
     - VP9 WebM playback with background decoding; debug builds also support
       MP4/H.264 and WebM/VP8 source assets.
   * - ``crates/assets/src/audio.rs`` and ``audio/``
     - Music, sound, voice, and movie soundtracks through kira.
   * - ``crates/assets``
     - Sandboxed files, archives, fonts, JSX and module resolution, plus
       background image decoding and media workers.
   * - ``crates/engine/src/app.rs``
     - winit/gilrs desktop front end.
   * - ``crates/headless``
     - Scripted offscreen front end for tests and screenshots.
   * - ``crates/engine-core/src/self_voicing.rs``
     - System speech service integration and retries.
   * - ``crates/engine-core/src/dev_console/``
     - Debug desktop console and live inspectors.
   * - ``crates/common``
     - UI descriptions, serialized callback IDs, speech snapshots, worker
       wakeups, diagnostic records, numeric helpers, and platform data paths.

Working on a domain
-------------------

Run ``cargo check -p deflorta-ui`` or ``cargo test -p deflorta-js-bridge`` to
check one domain without rebuilding the platform front end. Use
``cargo test --workspace --all-features`` for integration verification.
The ``dev-console`` feature on ``deflorta`` forwards diagnostics support to
the domain crates. Shader generation belongs to engine-core; JavaScript
transformation and minification belong to js-bridge, so either build script
can run independently.

Keep shared contracts free of VM, GPU, windowing, and media dependencies.
Cross-domain behavior belongs in engine-core, rather than introducing
dependencies from UI or assets back to the coordinator. The release profile
still uses fat LTO for optimized binaries; this split improves compilation
scheduling and incremental rebuild boundaries, not final-link parallelism.

Frame and event model
---------------------

1. An input event or due timer arrives. ``Engine`` creates a JavaScript object
   directly and calls the registered ``dispatch(event)`` function.
2. The microtask queue drains. Async story code runs until its next awaited
   engine interaction, such as dialogue, a menu, or a pause.
3. The registered ``flush()`` runs UI and music hooks. Multiple invalidations
   in one turn produce one ``native.ui.commit(tree, options)``. Native calls
   read values directly and queue typed Rust commands.
4. Rust applies commands in order, reconciles the tree, lays it out, and draws.
   Animation and typewriter work drive redraws; when idle, the event loop
   waits for input or timers.

Animation interpolation, text reveal, and hover styles remain in Rust, so
they do not call JavaScript once per frame. UI commits still read the whole
element tree, while native reconciliation retains unchanged layout caches.

Platform status and future work
-------------------------------

Desktop launchers support Linux, Windows, and macOS. Android exports are
implemented with GameActivity, native libraries, touch/keyboard input,
surface recreation, and private-file storage; see :doc:`../guides/android`.
The repository does not provide an iOS launcher or export workflow.

Project creation, checks, bundling, publishing, translations, editor
declarations, the debug developer console, and accessibility/self-voicing
are implemented. Future directions include hot reload with replay to the
current line, automated launcher builds for more platforms, linting for
unserializable state, additional codecs such as AV1/Opus, hardware video
decoding, and Steam/Discord integrations.

Continue with :doc:`runtime`, :doc:`rendering`, or :doc:`distribution`.
