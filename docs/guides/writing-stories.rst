Write stories that save and roll back correctly
===============================================

Game code is ordinary JavaScript in ES modules. Import the public API from
``"deflorta"`` and put the entry module in ``main.js``.
See :doc:`../reference/scripting` for the function reference.

Labels, dialogue, and choices
-----------------------------

Define labels at module scope. ``start`` begins a new game; the optional
``splashscreen`` label runs at boot. ``jump(name)`` transfers control to another
label; ``await call(name)`` runs a label and returns to the caller.

Characters are reusable speakers. Call one as ``await eileen("Hello!")`` or
as an awaited tagged template. Use ``say(text)`` for narration.

.. code-block:: javascript

   import {
     character, defaults, jump, label, menu, say, store,
   } from "deflorta";

   defaults({ trust: 0 });
   const eileen = character("Eileen", { color: "#f4b6d2" });

   label("start", async () => {
     await eileen`Hi there! {w}Nice to {b}meet{/b} you.`;
     const answer = await menu("Well?", [
       ["Hello!", "hi"],
       ["...", "silent"],
     ]);
     if (answer === "hi") store.trust += 1;
     jump("next");
   });

   label("next", async () => {
     await say(store.trust ? "You made a friend." : "A quiet beginning.");
   });

Menu entries can also be plain strings or objects with ``text``, ``value``, and
``if`` fields. ``prompt(question, options)`` asks for text input.
``pause(seconds)`` waits for time; ``pause()`` waits for a click.

Keep story state in store
-------------------------

Loading and rollback restore a snapshot and re-run the current label with
recorded inputs. Follow these rules so replay reaches the same result:

* Keep mutable story state in ``store``, with defaults declared using
  ``defaults({...})``. Values must be JSON serializable.
* Use ``random()`` or ``randInt(min, max)`` for story randomness. Avoid
  ``Math.random()`` and branching on the current date or time.
* Await engine functions such as ``say``, ``menu``, ``pause``, and ``call``.
  Arbitrary asynchronous work is outside the replay model.

Local variables computed from deterministic inputs inside a label are fine.
Mutable module variables are not restored by loading or rollback.

Use ``persistent`` for data shared by all playthroughs, such as unlocked
endings, and call ``savePersistent()`` after changing it. It is separate from
the state rolled back with a story. Keep UI-only state in hooks, as described
in :doc:`custom-screens`.

See :ref:`replay-model` for the engine's snapshot and checkpoint design.

Add images, transitions, and sound
----------------------------------

Put ``bg room.png`` and ``eileen happy.png`` in your game's ``images/``
directory, and your music in ``audio/``. Then import the scene functions:

.. code-block:: javascript

   import { dissolve, music, right, scene, show } from "deflorta";

   // Inside a label, before the next awaited line:
   scene("bg room", { with: dissolve(1) });
   show("eileen happy", { at: right });
   music.play("audio/theme.wav", { loop: true, fadeIn: 1 });

The first word of an image name is its tag: showing ``eileen sad`` replaces
``eileen happy``. Use ``image(name, src, options)`` to map a name to another
path. ``scene()`` clears the scene, and ``hide(tag)`` removes a sprite.

``layeredImage(tag, layers)`` combines a base with groups of attributes.
For example, the demo declares outfit and face groups and an optional blush
layer, then selects them with ``show("eileen sad blush")``. Remove the optional
attribute with ``show("eileen -blush")``. See ``game/main.js`` for the complete
declaration.

Use ``voice(file)`` to associate a recording with the next dialogue line,
and ``sound.play(file)`` for one-shot effects. :doc:`../reference/assets`
lists asset layout and supported movie formats.

Format dialogue
---------------

Text tags work in dialogue, menus, and ``RichText``. For example,
``{b}bold{/b}`` changes style and ``{w}`` waits for a click within a line.
Use ``{{`` for a literal opening brace. See :ref:`text-tags` for the full list.

For a full-screen page of accumulating dialogue, use ``nvlNarrator`` or a
character declared with ``{ nvl: true }``. Clear the page with ``nvlClear()``.

Check as you write
------------------

Run ``deflorta check mygame`` to find syntax errors, broken imports, missing
labels or assets, invalid translations, and likely nondeterministic code.
It also runs the scripts without a window to catch startup errors.
``--no-boot`` limits checking to static analysis; computed paths and values
cannot always be checked. Use :doc:`testing` to exercise the story itself.
