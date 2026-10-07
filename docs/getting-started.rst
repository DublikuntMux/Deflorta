Build the engine and make your first game
=========================================

This walkthrough starts in the Deflorta repository and ends with a playable
game. Desktop runtimes support Linux, Windows, and macOS. For Android, first
build a host distribution, then follow :doc:`guides/android`.

Install build prerequisites
---------------------------

You need:

* A Rust toolchain installed through rustup. The repository's
  ``rust-toolchain.toml`` selects nightly.
* Python 3.9 or newer for the distribution script.
* clang and libclang for bindgen. SpiderMonkey is downloaded as a prebuilt
  library for common targets; source builds also need Python and make.
* On Linux, the Speech Dispatcher development library, for example
  ``libspeechd-dev`` on Debian or Ubuntu.

These are engine build requirements. Players run the published game;
they do not need Rust or the development CLI.

Build a complete distribution
-----------------------------

From the repository root:

.. code-block:: sh

   python3 scripts/build-dist.py
   dist/deflorta run game

On Windows, use ``dist/deflorta.exe``. The second command opens the included demo.
Use Start to begin the story; :doc:`reference/controls` lists the controls.

The script assembles the release CLI, debug and release launchers, and game
templates under ``dist/``. Keep that directory together: the CLI locates
runtimes and templates beside its executable, independently of your working
directory. An individual ``cargo build`` does not assemble this distribution.

.. note::

   The examples below use ``deflorta``. Add the distribution's root directory
   to your PATH, or replace it with the path to ``dist/deflorta``.

Create and run a project
------------------------

.. code-block:: sh

   deflorta create mygame --title "My Game"
   deflorta check mygame
   deflorta run mygame

``create`` writes a playable story, fonts and their license, editor declarations
(``deflorta.d.ts``), and ``jsconfig.json``. Open ``mygame/main.js`` in your
editor. Completion and JavaScript type checking use the generated declarations.
Refresh them after an engine update with ``deflorta types mygame``; this command
preserves an existing editor configuration.

Write a small story
-------------------

Replace ``mygame/main.js`` with the following. It uses no images or audio, so
you can run it immediately with the starter project's fonts.

.. code-block:: javascript

   import {
     configure, defaults, character, label, menu, say, store,
   } from "deflorta";

   configure({
     id: "my-game",
     title: "My Game",
     version: "1",
     width: 1280,
     height: 720,
     font: "Noto Sans",
   });
   defaults({ trust: 0 });
   const guide = character("Guide", { color: "#f4b6d2" });

   label("start", async () => {
     await guide("Welcome to your first story.");
     const answer = await menu("What do you say?", [
       ["Hello!", "hi"],
       ["Stay quiet", "silent"],
     ]);
     if (answer === "hi") store.trust += 1;
     await say("Your story starts here.");
   });

Run ``deflorta check mygame`` again, then ``deflorta run mygame`` to play it.
The ``start`` label begins a new game. Each awaited line or choice waits for
the player's input.

Share the game
--------------

.. code-block:: sh

   deflorta publish mygame

The result is ``mygame/dist/<os>-<arch>/``. Share the whole folder; players
launch the executable named after your game's id. See
:doc:`guides/publishing` for other targets and output options.

Continue with :doc:`guides/writing-stories` to add scenes and saved state,
or :doc:`guides/custom-screens` to change the interface.
