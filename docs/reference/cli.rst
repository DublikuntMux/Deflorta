Command-line reference
======================

Run ``deflorta --help`` or ``deflorta <command> --help`` for built-in help.
Except for ``create`` and ``info``, the project path defaults to the current
directory. ``run`` also accepts a ``.dm`` archive.

Create and edit
---------------

.. code-block:: text

   deflorta create PATH [--title TITLE] [--id ID]
   deflorta types [PATH]

``create`` makes a playable starter project. The title defaults to the
directory name; the save-directory id defaults to a slug of the title.
``types`` writes ``deflorta.d.ts`` and creates ``jsconfig.json`` if missing.

Check and run
-------------

.. code-block:: text

   deflorta check [PATH] [--no-boot]
   deflorta run [PATH] [--test SCRIPT.json] [-v | -vv]

``check`` runs static analysis and startup verification. ``--no-boot`` skips
startup verification. ``run`` compiles source scripts before starting the
host's debug launcher. It reads source assets directly, without conversion
or a temporary project copy; only compiled scripts use temporary files. ``--test``
selects headless play. ``-v``/``--verbose`` enables debug logging and ``-vv``
enables trace; ``RUST_LOG`` overrides them. See :doc:`../guides/debugging`
and :doc:`../guides/testing`.

Bundle and inspect
------------------

.. code-block:: text

   deflorta bundle [PATH] [-o FILE] [--level N] [--no-minify] [--emit-js FILE]
   deflorta info ARCHIVE

.. list-table:: Bundle options
   :header-rows: 1
   :widths: 30 70

   * - Option
     - Behavior
   * - ``-o``, ``--output FILE``
     - Archive path; defaults to ``<project>/build/game.dm``.
   * - ``--level N``
     - Compression effort from 1 to 12; default 12. Level 1 is fast LZ4,
       levels 2–12 are LZ4HC.
   * - ``--no-minify``
     - Keep bundled JavaScript readable.
   * - ``--emit-js FILE``
     - Also write the combined script to a separate file.

Bundling re-encodes images, video, and audio using FFmpeg while preserving
asset paths. ``info`` lists an archive's contents. See :doc:`../guides/publishing`.

Publish
-------

.. code-block:: text

   deflorta publish [PATH] [-o DIR] [--platform PLATFORM] [--debug]
     [--name NAME] [--level N] [--android-package ID]
     [--android-format apk|aab] [--android-version-code N]

.. list-table:: Publish options
   :header-rows: 1
   :widths: 30 70

   * - Option
     - Behavior
   * - ``-o``, ``--output DIR``
     - Output folder; defaults to ``<project>/dist/<os>-<arch>``.
   * - ``--platform PLATFORM``
     - Select an installed runtime folder; defaults to the host platform.
   * - ``--debug``
     - Export the debug runtime instead of release.
   * - ``--name NAME``
     - Executable or Android artifact name; defaults to the game's id.
   * - ``--level N``
     - Archive compression level, 1–12; default 12.
   * - ``--android-package ID``
     - Android application id; defaults to ``org.deflorta.game_<game id>``
       with hyphens replaced by underscores.
   * - ``--android-format apk|aab``
     - Android package format; defaults to ``apk``.
   * - ``--android-version-code N``
     - Android store version code; defaults to 1. Allowed range:
       1–2,100,000,000.

Startup checks use the host debug runtime even for another platform's export.
See :doc:`../guides/publishing` and :doc:`../guides/android`.

Translate
---------

.. code-block:: text

   deflorta translate update [LANGUAGES...] [-p PATH] [--prune]
   deflorta translate status [-p PATH]
   deflorta translate missing LANGUAGE [-p PATH]

``-p``/``--project`` defaults to the current directory. ``update`` preserves
translations and adds new text; without languages it updates existing tables.
``--prune`` removes obsolete entries. ``status`` reports progress and
``missing`` lists untranslated text. See :doc:`../guides/localization`.
