Story, scene, and engine API
============================

Import the API from ``"deflorta"``. This page is a compact reference; start
with :doc:`../guides/writing-stories` for examples. The
:download:`editor declarations <../../crates/js-bridge/runtime/deflorta.d.ts>`
contain complete types and additional exports. ``deflorta types`` refreshes
the declarations in a game project.

Story flow and state
--------------------

.. list-table::
   :header-rows: 1
   :widths: 45 55

   * - API
     - Purpose
   * - ``label(name, asyncFn)``
     - Declare a label. ``start`` begins a new game; ``splashscreen`` runs
       at boot.
   * - ``jump(name)``, ``await call(name)``
     - Transfer control, or run a label and return.
   * - ``await say(text)``, ``await say(who, text, { voice })``
     - Narration or dialogue, optionally with a voice recording.
   * - ``character(name, { color, nvl })``
     - Create a speaker callable with a string or a tagged template.
   * - ``voice(file)``
     - Associate a recording with the next dialogue line.
   * - ``nvlNarrator``, ``nvlClear()``
     - Narrate in full-screen accumulating NVL mode, or clear its page.
       A character can use ``{ nvl: true }`` too.
   * - ``await menu(prompt?, choices)``
     - Return a selected choice's value. Entries are strings,
       ``[text, value]``, or ``{ text, value, if }``.
   * - ``await prompt(question, { default, maxLength, allowEmpty })``
     - Ask for text input and return the trimmed answer.
   * - ``await pause(seconds?)``
     - Wait for time, or for a click if no time is given.
   * - ``await playMovie(src, { skippable })``
     - Play a full-screen movie until it ends or is skipped.
   * - ``checkpoint(kind, present, { record, rollback })``
     - Build a custom interaction such as a minigame. ``present`` receives
       a checkpoint with ``resolve(value)``; recorded values replay on load.
   * - ``store``, ``defaults({...})``
     - JSON-serializable story state and defaults for a new game.
   * - ``persistent``, ``savePersistent()``
     - Data shared by all playthroughs and its explicit save operation.
   * - ``random()``, ``randInt(min, max)``
     - Deterministic randomness. ``random`` returns a number in [0, 1);
       integer endpoints are inclusive.
   * - ``history``
     - Dialogue backlog, oldest first.

.. _text-tags:

Text tags
---------

Tags work in dialogue, menus, and ``RichText``. Close formatting tags with
the corresponding closing tag, for example ``{color=#f88}text{/color}``.

.. list-table::
   :header-rows: 1
   :widths: 45 55

   * - Tag
     - Effect
   * - ``{b}``, ``{i}``, ``{u}``, ``{s}``
     - Bold, italic, underline, strikethrough.
   * - ``{color=#f88}``, ``{font=Name}``
     - Text color or font family.
   * - ``{size=32}``, ``{size=+4}``, ``{size=*1.5}``
     - Absolute, additive, or multiplied font size.
   * - ``{ruby=furigana}base{/ruby}``
     - Ruby annotation above base text.
   * - ``{w}``, ``{w=0.5}``
     - Wait for a click, or pause for seconds during text reveal.
   * - ``{p}``
     - Wait, then insert a line break.
   * - ``{nw}``
     - Advance automatically.
   * - ``{fast}``
     - Reveal preceding text instantly.
   * - ``{{``
     - Literal opening brace.

Images and positions
--------------------

.. list-table::
   :header-rows: 1
   :widths: 45 55

   * - API
     - Purpose
   * - ``image(name, src, { zoom })``
     - Declare an image. The first word of its name is its tag.
   * - ``layeredImage(tag, layers)``
     - Compose attribute groups. Select with ``show("eileen sad blush")``;
       remove an optional attribute with ``show("eileen -blush")``.
   * - ``scene(name?, { with })``
     - Clear the scene and optionally set a background.
   * - ``show(name, { at, with, zorder, transform })``
     - Show a sprite; another image with the same tag replaces it.
   * - ``hide(tag, { with })``
     - Remove a sprite.
   * - ``left``, ``center``, ``right``, ``truecenter``
     - Built-in anchor-based positions.
   * - ``offscreenleft``, ``offscreenright``
     - Positions outside the visible scene.
   * - ``at(x, y, { zoom, rotate })``
     - Custom anchor-based position.
   * - ``preload(...names)``
     - Decode images ahead of time.

Transitions and transforms
--------------------------

Pass transitions as ``with`` options:

.. code-block:: javascript

   scene("bg room", { with: dissolve(1) });
   show("eileen", { at: left, with: move() });

Available transition builders include ``dissolve``, ``fade``, ``moveinleft``,
``moveinright``, ``moveoutleft``, ``moveoutright``, and ``zoomin``.
``move(dur)`` animates a shown image to its new position.
``imageDissolve(mask, dur, ramp)``, ``wipeleft``, ``wiperight``, ``wipeup``,
``wipedown``, and ``pixellate(dur, size)`` use reveal masks.

A custom transition has ``dur``, ``ease``, ``in``, and ``out`` fields; the
last two can define ``opacity``, ``x``, ``y``, ``scale``, and ``rotate``.

ATL-style transforms build a program interpreted by the native engine:

.. code-block:: javascript

   atl().linear(1, { x: 50 }).ease(1, { y: -10 }).pause(0.5).repeat()

Transform properties include ``x``, ``y``, ``opacity``, ``scale``, ``rotate``,
and ``crop``. Easing names include ``linear``, ``ease``, ``easeIn``,
``easeOut``, and ``bounce``. Use ``parallel(a, b)`` to combine programs,
or the ``shake()`` and ``bob()`` helpers.

Audio
-----

``music.play(file, { loop, fadeIn, fadeOut, volume })`` starts background
music; ``music.stop()`` stops it. Music is saved with the scene.
``sound.play(file)`` plays a one-shot effect, suppressed during load/rollback
replay. ``voice(file)`` sets the next line's voice recording.

Saves and game control
----------------------

.. list-table::
   :header-rows: 1
   :widths: 45 55

   * - API
     - Purpose
   * - ``canSave()``
     - Whether the story is waiting for the player and can be saved.
   * - ``saveGame(slot, { thumbnail })``, ``loadGame(slot)``
     - Save or load a named slot; return success. Thumbnails default to on.
       Slot names use letters, digits, hyphens, and underscores.
   * - ``saveInfo(slot)``
     - Return ``{ time, preview, thumbnail }``, or null for an empty slot.
   * - ``deleteSave(slot)``
     - Delete a slot and its thumbnail.
   * - ``quickSave()``, ``quickLoad()``, ``autosave()``
     - Quick-save/load, or save to the oldest autosave slot.
   * - ``rollback()``, ``rollbackTo(historyEntry)``
     - Replay to a previous checkpoint or backlog entry; return success.
   * - ``newGame(start?)``, ``endGame()``
     - Begin a game at ``start`` (default ``"start"``), or return to the
       main menu.
   * - ``advance()``, ``toggleSkip(on?)``, ``isSkipping()``, ``inGame()``
     - Advance the current interaction and manage/query play state.

The default save interface has nine manual pages plus autosave and quick-save
pages. Saves include thumbnails. When replay detects incompatible story
changes, recovery restarts the scene; removed labels or unsupported save
formats still prevent loading. See :ref:`replay-model`.

Configuration and services
--------------------------

Call ``configure(options)`` at module scope. ``config`` exposes the current
configuration. Common fields are:

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Field
     - Meaning
   * - ``id``, ``title``, ``version``
     - Per-game save directory id, displayed title, and version stored in saves
       and logs. Android exports also use the version name.
   * - ``width``, ``height``
     - Virtual resolution, scaled and letterboxed to the window.
   * - ``font``
     - Default family loaded from the game's ``fonts/`` directory.
   * - ``textSpeed``
     - Dialogue characters per second; 0 reveals instantly.
   * - ``skipDelay``
     - Skip delay in milliseconds.
   * - ``clearColor``
     - Renderer clear color.
   * - ``autosave``
     - Set false to disable autosaves.
   * - ``languages``
     - Preferences language entries: ``{ id, name }``. Null id is the
       source language.
   * - ``menuBackground``, ``menuVideo``
     - Main-menu background image or looping movie.

``prefs`` contains player preferences. Call ``savePrefs()`` to save and apply
changes. ``setLanguage(id)``, ``translations(language, table)``, and ``_(text)``
provide localization. ``keymap`` and ``actions`` customize input behavior.

``on(event, fn)`` subscribes to an engine event and returns an unsubscribe
function. ``setTimer(ms, fn)`` and ``clearTimer(id)`` manage timers; global
``setTimeout(fn, ms)`` and ``clearTimeout(id)`` are available too.
``readText(path)`` reads game text files or returns null. ``storage`` provides
per-game JSON ``read``, ``write``, ``remove``, and ``list`` operations.
Reads and listings use a session snapshot and see queued writes immediately.
``write`` and ``remove`` queue disk persistence; ``remove`` returns whether
an entry existed in the snapshot. Disk failures are logged and reported to
``on("error", fn)``; the affected snapshot entry returns to its persisted
value unless a newer operation is pending. Normal shutdown finishes queued
writes. Changes made outside the engine are read on the next session.

``native`` exposes low-level ``app``, ``audio``, ``ui``, ``timers``, ``files``,
and ``storage`` modules, plus logging and runtime connection functions.
Examples are ``native.app.quit()`` and ``native.audio.voice(file)``.
Events and UI trees cross as direct JavaScript values; saves use JSON.
See :doc:`../architecture/runtime` for the boundary and callback lifetimes.
