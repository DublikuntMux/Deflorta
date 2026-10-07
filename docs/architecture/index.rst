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

Web builds, 3D rendering, and Live2D are outside the current scope.

Subsystems
----------

Within ``crates/engine/src/``:

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Path
     - Responsibility
   * - ``script/``
     - SpiderMonkey host, modules, microtasks, typed native calls, and direct
       JavaScript values.
   * - ``engine.rs``
     - Route input into JavaScript, apply commands, own timers/UI/audio,
       gate commits on image decoding, and capture save thumbnails.
   * - ``ui/``
     - Retained-tree reconciliation, taffy layout, animation programs,
       typewriter, rich text, scrolling, focus, widgets, and accessibility.
   * - ``render/``
     - wgpu quads, glyphon text, and offscreen capture.
   * - ``video/`` and ``video.rs``
     - MP4/H.264 and WebM/VP8/VP9 playback with background decoding.
   * - ``audio.rs`` and ``audio/``
     - Music, sound, voice, and movie soundtracks through kira.
   * - ``assets.rs``
     - Sandboxed file access and background image decoding.
   * - ``app.rs``
     - winit/gilrs desktop front end.
   * - ``headless.rs``
     - Scripted offscreen front end for tests and screenshots.
   * - ``self_voicing.rs``
     - System speech service integration and retries.
   * - ``dev_console/``
     - Debug desktop console and live inspectors.

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
