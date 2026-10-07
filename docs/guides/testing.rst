Automate playthroughs and screenshots
=====================================

Headless mode plays scripted input against an offscreen renderer without
opening a window. Use it for regression tests and store or press screenshots.
It still needs a working graphics backend; it is not a static script checker.

Run a test
----------

From the Deflorta repository, build the distribution first, then run:

.. code-block:: sh

   dist/deflorta run game --test tests/demo.json
   dist/deflorta bundle game
   dist/deflorta run game/build/game.dm --test tests/demo.json

The demo script plays through the demo and writes screenshots to
``target/shots/``. The archive command exercises the packaged game too.
For Rust and CLI regression tests, use ``cargo test --workspace``.

Write input steps
-----------------

A test file is a JSON list. Coordinates use the game's virtual resolution.
This example waits for rendering, captures a screenshot, and presses Enter:

.. code-block:: json

   [
     { "wait": 500 },
     { "shot": "target/shots/start.png" },
     { "key": "Enter" }
   ]

.. list-table:: Supported steps
   :header-rows: 1
   :widths: 55 45

   * - Step
     - Meaning
   * - ``{ "wait": 500 }``
     - Wait in milliseconds.
   * - ``{ "move": [640, 360] }``
     - Move the pointer.
   * - ``{ "click": [640, 360] }``
     - Click at a position.
   * - ``{ "click": [640, 360], "button": "right", "release": false }``
     - Press a particular button and leave it held.
   * - ``{ "release": true }``
     - Release the held pointer button.
   * - ``{ "key": "Enter" }``
     - Send a key.
   * - ``{ "key": "Control", "down": true }``
     - Hold a key; use ``down: false`` to release it.
   * - ``{ "type": "text" }``
     - Enter text.
   * - ``{ "wheel": -1 }``
     - Send a wheel event.
   * - ``{ "shot": "out.png" }``
     - Save a screenshot.

Isolate saved data
------------------

Preferences and saves affect repeatability. On Linux, set ``XDG_DATA_HOME``
to a fresh temporary directory to isolate a run:

.. code-block:: sh

   test_data_dir=$(mktemp -d)
   XDG_DATA_HOME="$test_data_dir" dist/deflorta run game --test tests/demo.json

Without isolation, the demo data is normally in
``~/.local/share/deflorta/deflorta-demo`` on Linux.

``tests/save-delete.json`` saves a manual slot, confirms deletion, and removes
its thumbnail; it expects slot 1 on page 1 to be empty. ``tests/save-hover.json``
saves the same slot and exercises its preview, delete-button tooltip, and
tooltip dismissal. With ``RUST_LOG=deflorta=trace``, steps 16–28 of the hover
script should produce no ``Committing UI tree`` entries.
