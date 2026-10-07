Deflorta documentation
======================

Deflorta is a visual novel engine written in Rust. Write your story, characters,
menus, save screens, and preferences in JavaScript. Use JSX to build native
interfaces without a browser, npm packages, or a separate JavaScript build step.

Start with :doc:`getting-started` to build the engine and make a playable game.
The repository's ``game/main.js`` demonstrates dialogue, choices, layered images,
transitions, audio, movies, translations, and custom screens.

Find what you need
------------------

* **Make a game:** :doc:`guides/writing-stories` and :doc:`guides/custom-screens`.
* **Reach players:** :doc:`guides/localization`, :doc:`guides/accessibility`,
  and :doc:`guides/publishing`.
* **Look up an API:** :doc:`reference/scripting`, :doc:`reference/ui`,
  and :doc:`reference/cli`.
* **Work on the engine:** :doc:`architecture/index` and
  :doc:`architecture/runtime`.
* **Edit this handbook:** :doc:`documentation`.

.. toctree::
   :maxdepth: 1
   :caption: Get started

   getting-started

.. toctree::
   :maxdepth: 1
   :caption: Make and ship a game

   guides/writing-stories
   guides/custom-screens
   guides/localization
   guides/accessibility
   guides/publishing
   guides/android
   guides/debugging
   guides/testing

.. toctree::
   :maxdepth: 1
   :caption: Reference

   reference/cli
   reference/scripting
   reference/ui
   reference/assets
   reference/archive-format
   reference/controls

.. toctree::
   :maxdepth: 1
   :caption: Engine design

   architecture/index
   architecture/runtime
   architecture/rendering
   architecture/distribution

.. toctree::
   :maxdepth: 1
   :caption: Contribute

   documentation

License
-------

The engine is MIT licensed; see :download:`LICENSE.md <../LICENSE.md>`.
The demo uses Noto Sans fonts with their own license in
``game/fonts/LICENSE-noto.txt``.
