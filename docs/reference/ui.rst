Screen and component reference
==============================

All public components and hooks are imported from ``"deflorta"``.
See :doc:`../guides/custom-screens` for a complete example and lifecycle rules.
The :download:`editor declarations <../../crates/js-bridge/runtime/deflorta.d.ts>`
provide the full types.

Native JSX components
---------------------

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Component
     - Use and component-specific props
   * - ``View``
     - Flexbox container; accepts children.
   * - ``Grid``
     - Grid container with ``columns``.
   * - ``ScrollView``
     - Scrollable container; ``startAtEnd`` starts at the end.
   * - ``Text``
     - Text from children; ``cps`` controls typewriter speed.
   * - ``RichText``
     - Text with markup tags from children.
   * - ``Image``
     - ``src``, ``hoverSrc``, ``onPress``, ``alt``, ``fit``, and ``anchor``.
   * - ``Pressable``
     - A container with an ``onPress`` handler.
   * - ``Slider``
     - ``value``, ``onValueChange``, ``min``, ``max``, and ``step``.
   * - ``TextInput``
     - ``value``, ``onChangeText``, ``onSubmit``, ``placeholder``, and
       ``maxLength``.
   * - ``Video``
     - ``src``, ``loop``, ``onEnd``, and ``fit``.

Screens and hooks
-----------------

``screen(name, render, { z, modal, keys })`` defines or replaces a screen.
``showScreen(name, props)``, ``hideScreen(name)``, and ``isShown(name)`` manage
the screen stack. Game screens participate in saves and rollback.

Hooks are ``useState``, ``useReducer``, ``useEffect``, ``useRef``, ``useMemo``,
and ``useCallback``. Hook state belongs to the mounted component; use ``store``
for saved story state. ``invalidate()`` schedules rendering after external
state changes.

``theme`` supplies colors and sizes for default screens. ``tooltip()`` returns
the current tooltip.

Notifications
-------------

``notify(message, options?)`` shows a compact pill at the top center of the
screen and returns a handle for updating or dismissing that notification:

.. code-block:: javascript

   import { notify } from "deflorta";

   const notice = notify("Downloading…", { state: "loading" });
   notice.update({ message: "Finishing download…" });
   notice.update({ message: "Download complete", state: "success" });
   // Or dismiss it explicitly:
   notice.dismiss();

``state`` is ``"info"`` (plain text), ``"loading"`` (spinner), ``"success"``
(check mark), or ``"error"`` (warning mark). ``duration`` is seconds until
dismissal, or ``null`` to keep the pill open. The default is two seconds;
loading notifications stay open until updated or dismissed.

``handle.update({ message?, state?, duration? })`` keeps the same notification
identity and restarts its lifetime. Changing state uses the new state's
default duration unless the update specifies one. ``handle.id`` is read-only.
Updating a dismissed or expired handle does not bring its notification back.
Each handle operates independently. Multiple notifications form an equal-width
stack below the top edge, newest first. Only the top and bottom outer corners
are rounded, using the same curves as a single pill; middle rows are square.
The stack shows as many notifications as fit on the screen. Text wraps on
narrow screens, with an ellipsis for messages longer than three lines.
Notifications survive screen changes and remain outside saves and rollback.
They announce through screen readers and self-voicing. The engine's self-voicing
startup indicator uses this same notification system.

Common element props
--------------------

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Prop
     - Meaning
   * - ``key``
     - Stable identity for layout caches, focus, animations, and component
       state. Use for children that can be inserted, removed, or reordered.
   * - ``style``, ``hover``
     - Objects or nested arrays merged left to right. Hover overrides apply
       while hovered or focused.
   * - ``onPress``
     - Activation handler.
   * - ``tooltip``
     - Tooltip text on hover or focus.
   * - ``label``, ``live``
     - Accessible name and polite announcement of subtree changes.
   * - ``focusable``, ``autofocus``
     - Participate in focus navigation, or request initial focus.
   * - ``enter``, ``exit``, ``move``
     - Native animations with ``dur``, ``ease``, ``opacity``, ``x``, ``y``,
       ``scale``, ``rotate``, and ``mask`` fields.
   * - ``transform``
     - ATL-style animation program.

Images accept ``fit: "fill" | "cover" | "contain"`` and ``anchor: [x, y]``;
``[0.5, 1]`` is bottom center. ``alt`` names the image or its action;
``alt: ""`` marks it decorative.

Text accepts ``cps`` and ``tooltipText: true``. The latter displays the native
tooltip immediately and hides when empty, without a JavaScript re-render.
Scroll containers accept ``startAtEnd``.

Style properties
----------------

Sizes are pixels or percentages such as ``"50%"`` where supported by the
declarations. Padding and margin accept a number, ``[vertical, horizontal]``,
or ``[top, right, bottom, left]``. Colors use ``#rgb``, ``#rgba``,
``#rrggbb``, or ``#rrggbbaa``.

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Group
     - Properties
   * - Position and size
     - ``position``, ``left``, ``top``, ``right``, ``bottom``, ``width``,
       ``height``, ``minWidth``, ``minHeight``, ``maxWidth``, ``maxHeight``.
   * - Spacing
     - ``padding``, ``margin``, ``gap``.
   * - Flexbox
     - ``flexDirection``, ``flexWrap``, ``flexGrow``, ``flexShrink``,
       ``justifyContent``, ``alignItems``, ``alignSelf``.
   * - Grid and visibility
     - ``gridColumns``, ``gridRows``, ``display`` (including ``"none"``),
       ``overflow`` (``"visible"``, ``"hidden"``, ``"scroll"``).
   * - Appearance
     - ``background``, ``radius``, ``borderWidth``, ``borderColor``,
       ``opacity``, ``scale``, ``rotate``.
   * - Widgets
     - ``fillColor``, ``thumbColor``, ``thumbSize``, ``scrollbarColor``.
   * - Inherited text
     - ``color``, ``fontSize``, ``fontFamily``, ``fontWeight``, ``italic``,
       ``lineHeight``, ``textAlign``, ``textShadow: { color, x, y }``.

Use :doc:`../guides/accessibility` to choose accessible names and
:doc:`../architecture/rendering` for native layout and animation behavior.
