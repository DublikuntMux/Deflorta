Make a game accessible
======================

Deflorta exposes its retained UI through AccessKit and provides optional
self-voicing using the system speech service. Name controls clearly and test
them with keyboard navigation as well as a mouse or touchscreen.

Screen readers
--------------

The desktop window exposes control names, roles, bounds, focus, slider ranges,
and text-field values through ``accesskit`` and ``accesskit_winit``. Screen
readers can focus, activate, and edit controls. Modal screens expose their own
content while open. Dialogue uses polite live regions and exposes complete
text while the typewriter animation runs.

Name custom controls
--------------------

Use ``label`` for sliders, text fields, or controls with ambiguous visible text.
Use ``alt`` to describe the action of an image-only control; an empty ``alt``
marks a decorative image. Tooltips supply a name when other text is absent.
Translate accessible names like visible interface text:

.. code-block:: jsx

   import { _, Image, Slider, prefs, quickSave, savePrefs } from "deflorta";

   function Settings() {
     return <>
       <Image src="images/save.png" hoverSrc="images/save-hover.png"
         onPress={quickSave} alt={_("Save")} />
       <Slider value={prefs.musicVolume} onValueChange={value => {
         prefs.musicVolume = value;
         savePrefs();
       }} label={_("Music volume")} />
     </>;
   }

Set ``live: true`` on a custom dialogue or status container to announce
changes through the player's screen reader. Arrow keys and gamepads move
focus spatially between focusable controls, which use their ``hover`` style
when focused. :doc:`../reference/controls` lists the default bindings.

Enable self-voicing
-------------------

Press F6 on any game screen, or select Preferences → Self-voicing. The setting
is saved. Self-voicing reads dialogue, changed screen content, and hovered or
focused controls. Code can change it through ``prefs.selfVoicing`` followed
by ``savePrefs()``, or toggle it with ``actions.selfVoicing()``.

Linux players need Speech Dispatcher running with a configured speech engine.
Windows and macOS use their system voices; Android needs an installed speech
engine and voice data. Failed initialization is logged while the game
continues. The engine retries after 2, 4, and 8 seconds to allow slow speech
services to become ready. Disabling self-voicing cancels pending retries.

Headless tests do not initialize speech. In a debug desktop build,
``accessibility`` in the developer console inspects the current tree;
``accessibility --window`` opens a live inspector. See :doc:`debugging`.
