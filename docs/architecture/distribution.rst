Workspace, distributions, and archives
======================================

Workspace crates
----------------

The Cargo workspace separates tooling, domain libraries, and platform entry
points:

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Crate
     - Responsibility
   * - ``crates/assets``
     - Shared files, archives, font discovery, module resolution, and JSX
       compilation, plus image loading, audio, video, and background media
       workers. Exposes ``GameFiles`` for directory/archive reads and seeks.
   * - ``crates/engine``
     - Public ``deflorta`` entry points and desktop/Android window integration.
   * - ``crates/common``
     - Lightweight contracts and utilities shared by engine domains.
   * - ``crates/ui``
     - Retained UI, layout, text, animation, input, and accessibility.
   * - ``crates/js-bridge``
     - SpiderMonkey bridge, embedded JavaScript runtime, and save storage.
   * - ``crates/engine-core``
     - Engine coordination, rendering, self-voicing, and developer console.
   * - ``crates/headless``
     - Offscreen scripted execution and screenshots without the platform
       front end's event loop or gamepad/accessibility adapter dependencies.
   * - ``crates/cli``
     - clap-based developer CLI; uses assets and FFmpeg for media conversion
       without linking the engine, renderer, or JavaScript VM.
   * - ``crates/launcher-desktop``
     - Desktop game launcher, shipped to players.
   * - ``crates/launcher-android``
     - Android native runtime as ``libdeflorta.so``.

Scripts, fonts, images, audio, and video use the same ``GameFiles`` abstraction
for development directories and packed games.

The assets crate includes its file and media APIs without a ``media`` feature
gate. Its baseline codecs are WebP/PNG, VP9, and Vorbis in WebM/Ogg containers.
The ``debug-formats`` feature adds source-format decoders such as H.264, VP8,
JPEG, WAV, MP3, FLAC, and AAC. Desktop launchers enable it by default; the
distribution builder disables default features for release launchers and
explicitly enables ``debug-formats`` for debug Android launchers.

To build a desktop release manually, use
``cargo build -p deflorta-launcher-desktop --release --no-default-features``.
For an Android debug runtime, enable ``deflorta-launcher-android/debug-formats``.

Engine distribution layout
--------------------------

``scripts/build-dist.py`` assembles the release CLI, both desktop launcher
profiles, and template data. Windows executable names have an ``.exe`` suffix.

.. kroki::
   :type: graphviz
   :caption: Engine distribution layout, shown for Linux x86_64.
   :align: center

   digraph distribution {
       graph [bgcolor="transparent", rankdir=LR];
       node [shape=box, style="rounded,filled", fillcolor="#f1f5f9",
             color="#475569", fontcolor="#0f172a", fontname="sans-serif"];
       edge [color="#475569", arrowhead=none];
       root [label="dist/"];
       cli [label="deflorta\nDeveloper CLI"];
       target [label="target/"];
       platform [label="linux-x86_64/"];
       debug [label="debug/\ndeflorta-launcher"];
       release [label="release/\ndeflorta-launcher"];
       template [label="template/"];
       game [label="game/\nStarter files, fonts, license,\neditor declarations"];
       runtime [label="runtime/\nJS sources for analysis and\ntranslation extraction"];
       root -> { cli target template };
       target -> platform;
       platform -> { debug release };
       template -> { game runtime };
   }

Platform names follow ``<os>-<arch>``. The CLI resolves these directories
beside its executable, not from the working directory. Runtime folders can
also hold libraries and engine resources. ``publish`` copies all their contents
beside ``game.dm`` and renames the launcher to the game's id.

``run`` and startup inspection use the host debug launcher. Publishing selects
the requested release runtime, or debug with ``--debug``. Foreign-target
publishing still verifies startup through the host launcher.

With ``--android``, the script also adds ``template/android/`` and native
libraries in ``target/android-aarch64/<profile>/jniLibs/``. Optional x86_64
templates use ``android-x86_64``. See :doc:`../guides/android`.

JavaScript bundling
-------------------

The CLI follows static imports/re-exports from ``main.js``, renames bindings
to avoid collisions, and emits one game module. Namespace objects use live
getters. Engine modules remain embedded in the launcher.

Oxc provides parsing, semantic analysis, JSX transformation, and minification.
The CLI and runtime share a resolver, so development and bundled import paths
agree. Unsupported module features are listed in :ref:`module-paths`.

Game archives
-------------

``game.dm`` contains an LZ4-framed index followed by independent 128 KiB chunks.
Packing uses bounded batches; readers load only the requested chunk to keep
media seekable without unpacking the game.

See :doc:`../reference/archive-format` for the header, index, compression,
seeking, and validation rules. Use :doc:`../guides/publishing` to produce or
inspect an archive.
