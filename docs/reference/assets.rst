Project files, imports, and media
=================================

Project layout
--------------

.. kroki::
   :type: graphviz
   :caption: Game project files and asset folders.
   :align: center

   digraph project {
       graph [bgcolor="transparent", rankdir=LR];
       node [shape=box, style="rounded,filled", fillcolor="#f1f5f9",
             color="#475569", fontcolor="#0f172a", fontname="sans-serif"];
       edge [color="#475569", arrowhead=none];
       root [label="mygame/"];
       main [label="main.js\nEntry module"];
       types [label="deflorta.d.ts\nEditor declarations"];
       config [label="jsconfig.json\nEditor configuration"];
       images [label="images/\nBackgrounds and character images"];
       audio [label="audio/\nMusic, sound effects, voice"];
       movies [label="movies/\nMP4 or WebM video"];
       fonts [label="fonts/\nBundled fonts"];
       translations [label="tl/\n<language>.json translation tables"];
       root -> { main types config images audio movies fonts translations };
   }

An undeclared image name such as ``"bg room"`` resolves to
``images/bg room.png``.

All fonts in ``fonts/`` are loaded. Select the default family with
``configure({ font: "Noto Sans" })``. Bundle fonts and their licenses with
the project so text renders consistently on players' machines.

Project files and directories must be ordinary files, not symlinks. Bundling
rejects symlinks; runtime file access refuses symlink reads and directory
traversal, keeping reads within the game root.

.. _module-paths:

JavaScript module paths
-----------------------

Games are ES modules; use static default, named, or namespace imports and
re-exports. Paths name files exactly, including their extension:

.. code-block:: javascript

   import { say } from "deflorta";
   import { eileen } from "./characters.js";
   import "./chapter.js";
   import "../shared.js";
   import "/screens.jsx";

These are independent examples: choose paths that exist in your project.
Relative paths resolve from the importing module; a leading slash is relative
to the game root. ``"deflorta"`` and its built-in submodules are embedded
runtime modules.

JSX works in ``.js`` and ``.jsx`` modules and is compiled with Oxc's automatic
JSX runtime. npm package resolution, dynamic ``import()``, direct ``eval()``,
and TypeScript compilation are not supported. Direct eval is rejected with its
source location because bundling merges module scopes; use ordinary functions
to read local state. Declaration files provide editor types without a
TypeScript build.

Movies
------

Movies work with ``playMovie()``, the ``Video`` component, and
``configure({ menuVideo })``. Video supports looping and end callbacks.
Decoding uses pure Rust dependencies; FFmpeg and libvpx are not required.

.. list-table:: Supported formats
   :header-rows: 1
   :widths: 20 45 35

   * - Container
     - Video
     - Audio
   * - MP4
     - H.264, 8-bit 4:2:0, cropping and presentation-order B-frames.
     - AAC.
   * - WebM
     - VP8 or VP9. VP9 supports 8/10/12-bit and 4:2:0, 4:2:2, or 4:4:4
       chroma, displayed as 8-bit RGBA.
     - Mono or stereo Vorbis; duration metadata is required for soundtracks.

WebM files without audio play silently. Opus audio, AV1 video, and WebM alpha
channels are not supported.

Archives
--------

``.dm`` archives keep files seekable, including movie and audio streams.
Already-compressed media and blocks that do not shrink are stored verbatim.
See :doc:`../guides/publishing` to build one and
:doc:`archive-format` for its format.
