Default player controls
=======================

These bindings apply to the default game interface. Game code can customize
them through ``keymap``, ``actions``, and screen-specific keys.

.. list-table::
   :header-rows: 1
   :widths: 25 45 30

   * - Action
     - Keyboard / mouse
     - Gamepad
   * - Advance
     - Click, Enter, Space, wheel down
     - A
   * - Roll back
     - Wheel up, Page Up
     - LB
   * - Skip
     - Hold Ctrl; Tab toggles
     - Hold RT
   * - History
     - H
     - —
   * - Game menu
     - Esc, right click
     - B, Start
   * - Hide the interface
     - Middle click
     - Y
   * - Navigate menus
     - Arrow keys and Enter
     - D-pad / left stick and A
   * - Quick save / load
     - F5 / F9
     - —
   * - Fullscreen
     - F11
     - —
   * - Self-voicing
     - F6
     - —

In debug desktop builds, F12 toggles the developer console; see
:doc:`../guides/debugging`. Self-voicing is also available in Preferences;
see :doc:`../guides/accessibility`.

Stable key names
----------------

Keyboard bindings use character text (for example ``h`` and `` `` for Space)
or explicit named keys such as ``Enter``, ``Escape``, ``Control``, ``PageUp``,
``ArrowUp``, and ``F1``–``F35``. Navigation, editing, modifier, browser, and
common media keys keep their named strings across winit upgrades. Unsupported
named keys are ignored. Android's system Back key remains ``Escape``.

Desktop gamepads send independent key names through the same key event path:

* ``GamepadSouth``, ``GamepadEast``, ``GamepadNorth``, ``GamepadWest``,
  ``GamepadC``, ``GamepadZ`` (face buttons).
* ``GamepadLeftTrigger``, ``GamepadRightTrigger`` (LB/RB) and
  ``GamepadLeftTrigger2``, ``GamepadRightTrigger2`` (LT/RT).
* ``GamepadSelect``, ``GamepadStart``, ``GamepadMode``, ``GamepadLeftThumb``,
  ``GamepadRightThumb`` (menu and stick buttons).
* ``GamepadDPadUp``, ``GamepadDPadDown``, ``GamepadDPadLeft``,
  ``GamepadDPadRight`` and ``GamepadLeftStickUp``, ``GamepadLeftStickDown``,
  ``GamepadLeftStickLeft``, ``GamepadLeftStickRight`` (directions).

Left-stick directions become held keys beyond 60% deflection and release on
returning to the dead zone. D-pad and stick directions repeat after 400 ms,
then every 100 ms with ``repeat: true``. Other buttons do not repeat.
Disconnecting releases held keys. Connecting an idle controller does not
keep the window event loop awake. Gilrs does not support Android; Android
keyboard/controller keys reported by winit use the keyboard names above.

The default ``keymap`` binds South to ``confirm`` (native Enter activation),
East/Start to ``menu``, North to ``hideUi``, LB to ``rollback``, RT to
``skipHeld``, and directions to ``focusUp``/``focusDown``/``focusLeft``/
``focusRight``. These actions preserve native menu navigation, slider
adjustment, text-field submission, and screen Escape handlers. Screen-specific
gamepad keys take precedence. Games can change or delete each binding:

.. code-block:: javascript

   keymap.GamepadNorth = "history";
   keymap.GamepadWest = "quickSave";
   delete keymap.GamepadRightTrigger2;

Touch gestures
--------------

A tap acts as a left click on release. Holding one finger for 500 ms while
remaining within 12 logical pixels acts as a right click, normally opening
the game menu. An upward swipe of at least 60 logical pixels, with vertical
movement at least 1.5 times the horizontal movement, sends Page Up to the
existing rollback binding. A gesture does not also send a tap.

Gestures are disabled when the touch starts on a slider, text field, or inside
a scroll container. Sliders and fields receive the press immediately;
vertical dragging in scroll containers scrolls their contents, including when
the touch starts on a child button. Reaching a scroll boundary never rolls
back the story. Movement beyond the tolerance cancels a long press. Cancelled
touches do not click, and extra fingers cannot release the tracked finger.
