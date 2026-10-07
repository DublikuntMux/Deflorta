Find errors and inspect a running game
======================================

Start with ``deflorta check mygame``. It catches syntax and import errors,
missing exports, labels, assets, and invalid translation tables. It warns
about duplicate or unused labels, missing fonts, and nondeterministic story
code. The default startup check also catches exceptions before a window opens.

Capture logs
------------

Deflorta writes timestamped logs to stderr: startup, GPU and window setup,
audio, fonts, script modules, saves, loads, videos, warnings, and errors.
Game ``console`` messages use the ``deflorta::js`` log target. GPU validation
errors are logged instead of crashing the game.

.. code-block:: sh

   deflorta run mygame 2> deflorta.log
   deflorta run mygame -v 2> deflorta.log
   RUST_LOG=deflorta=trace,wgpu=warn deflorta run mygame

The default shows engine info, warnings, and errors. ``-v`` enables debug
details; ``-vv`` enables trace. ``RUST_LOG`` overrides verbosity. Include a
``RUST_LOG=deflorta=debug`` log when reporting a problem.

Use the developer console
-------------------------

Debug desktop launchers open a floating egui developer console. F12 hides or
reopens it; drag its title bar to move it and its edges to resize it.
It shows engine logs and JavaScript console messages, including
``console.debug``, with filtering, clearing, and automatic scrolling.

``deflorta run`` uses the host debug runtime. During engine development you
can also run the launcher directly:

.. code-block:: sh

   cargo run -p deflorta-launcher -- game

Enter runs a command, Shift+Enter inserts a line break, and Up/Down browse
command history. Results and exceptions appear in the log.

.. list-table:: Diagnostic commands
   :header-rows: 1
   :widths: 30 70

   * - Command
     - Purpose
   * - ``help [command]``
     - List commands or explain one.
   * - ``accessibility [--window]``
     - Inspect AccessKit node IDs, roles, names, properties, and focus.
   * - ``assets [--window]``
     - Inspect image decode/upload states, GPU textures, active audio/video,
       fonts, and JavaScript modules.
   * - ``unload <asset id>``
     - Unload an image, texture, or playback by its source/ID. Also accepts
       ``assets unload <asset id>``.
   * - ``stats [--window]``
     - Inspect process CPU, memory, disk I/O, redraw timings, GPU resources,
       and engine counts.

Commands print a snapshot by default. ``--window`` opens a separate inspector
that refreshes every 500 ms. These windows remain open when F12 hides the
console; close them individually with ×. The asset window has filters and
Unload buttons; the accessibility window expands nodes and their properties.

Inspect JavaScript state
------------------------

Other input runs in the live game's JavaScript global scope. Global
declarations persist between commands; the public game API is available as
``deflorta``:

.. code-block:: javascript

   deflorta.store
   deflorta.config.textSpeed = 0
   deflorta.jump("start")
   console.log(deflorta.prefs)

Game module locals remain scoped to their modules. Evaluation drains promise
jobs and commits updates. The console is excluded from release builds,
headless tests, and save thumbnails.

Understand asset and performance diagnostics
--------------------------------------------

Unreferenced images, preloads, failed loads, cached dimensions, and textures
expire after 60 seconds. Cleanup checks run every 5 seconds, including while
idle. Current and incoming scenes, hover images, and active transition masks
stay loaded. Finished audio handles are cleaned up too.

Forced unload releases CPU and GPU allocations together and discards late
decoder results. Images still referenced by the UI reload on the next draw;
forced audio/video unload stops playback. Fonts and JavaScript modules stay
loaded for the session and cannot be unloaded. Asset IDs preserve spaces and
query suffixes, for example ``assets unload "images/title screen.png?2"``.

CPU statistics cover the game process; 100% means one CPU core. They need a
second sample after warm-up. Redraw timings include inspector work. GPU memory
uses wgpu counters and allocation reports where supported; image/video sizes
are RGBA8 estimates. Hardware GPU utilization and execution time are
unavailable, and streamed assets have no full-file memory estimate.

Common fixes
------------

* **Missing runtime or template:** keep the complete engine distribution
  beside the CLI. Rebuild it with ``scripts/build-dist.py`` if necessary.
* **An import works differently than expected:** use exact file paths with
  extensions; see :ref:`module-paths` for unsupported module features.
* **A save or rollback changes the result:** keep mutable story data in
  ``store`` and use deterministic engine randomness; see
  :doc:`writing-stories`.
* **Self-voicing is silent:** check the platform speech service and installed
  voice data; see :doc:`accessibility`.
