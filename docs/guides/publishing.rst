Bundle and publish a desktop game
=================================

Use ``bundle`` to create an archive for testing or inspection. Use ``publish``
to make a complete folder players can run. Android has its own
:doc:`android` guide.

Build an archive
----------------

.. code-block:: sh

   deflorta check mygame
   deflorta bundle mygame
   deflorta info mygame/build/game.dm
   deflorta run mygame/build/game.dm

The default output is ``mygame/build/game.dm``. The bundler follows static
imports from ``main.js`` and combines game modules into one minified
``main.js``. Engine modules remain embedded in the launcher.

Use ``--no-minify`` to keep the script readable, ``--emit-js FILE`` to inspect
it separately, and ``-o FILE`` to choose the archive path:

.. code-block:: sh

   deflorta bundle mygame --no-minify --emit-js mygame/build/main.js

Bundling and publishing use LZ4HC level 12 by default. Levels 2–12 select HC
compression effort; level 1 selects fast compression. Choose another level
with ``--level N``. The archive supports direct reads and seeks, so movies
and audio stream without unpacking the game. See
:doc:`../reference/archive-format` for the format.

Tooling files and ``build/``, ``dist/``, and ``node_modules/`` directories are
excluded from game assets. Project symlinks are rejected. Import limitations
and asset paths are documented in :doc:`../reference/assets`.

Create a player distribution
----------------------------

.. code-block:: sh

   deflorta publish mygame

Publishing checks the project, builds ``game.dm``, boots the bundle to check
startup, and copies the selected runtime beside it. The default output is
``mygame/dist/<os>-<arch>/``. The launcher is renamed to the game's configured
id, with ``.exe`` on Windows.

Players launch that executable. It finds ``game.dm`` beside itself regardless
of the working directory. Distribute the entire output folder, including any
runtime libraries and resources.

Choose an output and runtime
----------------------------

.. code-block:: sh

   deflorta publish mygame -o release/mygame --name MyGame
   deflorta publish mygame --platform windows-x86_64
   deflorta publish mygame --debug

The CLI selects ``target/<platform>/release/`` beside its own executable;
``--debug`` selects the debug runtime. Debug desktop launchers include the
developer console. Release launchers exclude it and its dependencies.

The requested runtime must already be installed in the distribution. Startup
verification always uses the host's debug runtime, including when publishing
for another platform; it does not execute a foreign launcher.

Build engine distributions for other targets
--------------------------------------------

For engine development, the distribution script accepts:

.. code-block:: sh

   python3 scripts/build-dist.py --output engine-dist
   python3 scripts/build-dist.py --target x86_64-pc-windows-gnu --output windows-dist

Install the target and the required linker first. Cross builds produce a CLI
for that target as well as its launchers. The script honors Cargo's target
directory and replaces an existing output directory after successful
compilation. See :doc:`../architecture/distribution` for the directory layout.
