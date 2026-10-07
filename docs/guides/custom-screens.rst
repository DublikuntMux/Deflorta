Build and customize screens
===========================

Screens are JSX function components rendered into native UI elements. JSX
works in both ``.js`` and ``.jsx`` files, including ``main.js``. Deflorta
compiles it automatically; no npm package or separate build step is needed.

Register a screen
-----------------

.. code-block:: jsx

   import {
     screen, showScreen, View, Text, Pressable, useState,
   } from "deflorta";

   function Counter({ title }) {
     const [count, setCount] = useState(0);
     return <View style={{ padding: 24, gap: 12, flexDirection: "column" }}>
       <Text>{title}: {count}</Text>
       <Pressable onPress={() => setCount(value => value + 1)}>
         <Text>Add one</Text>
       </Pressable>
     </View>;
   }

   screen("counter", Counter, { z: 20 });
   showScreen("counter", { title: "Clicks" });

``screen`` defines or replaces a screen. ``showScreen`` adds it to the stack
with props; ``hideScreen`` removes it, and ``isShown`` checks its visibility.
Use the ``z``, ``modal``, and ``keys`` registration options to control ordering,
modal behavior, and screen-specific input. Shown game screens are saved and
rolled back with the scene.

Replace a default screen
------------------------

Register your component under a default screen's name to replace it.
Common names are ``say``, ``nvl``, ``choice``, ``input``, ``history``,
``quick_menu``, ``main_menu``, and ``game_menu``. The default implementations
in ``crates/engine/runtime/screens.js`` show the props each screen receives.
Change ``theme`` to adjust the colors and sizes used by the default screens.

Manage component state
----------------------

``useState``, ``useReducer``, ``useEffect``, ``useRef``, ``useMemo``, and
``useCallback`` track state by component identity and dependencies.

* Call hooks unconditionally at the top of a component or custom hook.
* Define component functions outside render functions.
* Use stable ``key`` values in lists to preserve state when items reorder.
* Hook setters schedule rendering automatically. After changing external
  state a component reads, call ``invalidate()``; story functions already do so.
* Effects run after native commits. Cleanup runs before changed dependencies
  take effect and when the component unmounts.

Hiding a screen unmounts it. Temporarily hiding the entire interface preserves
its state. Hook state is local to the UI and is not saved or rolled back;
put story state in ``store``.

Layout and interactions
-----------------------

Use ``View`` for flexbox, ``Grid`` for grids, ``ScrollView`` for scrolling,
and ``Text`` or ``RichText`` for text. Interactive components include
``Pressable``, ``Slider``, ``TextInput``, and clickable ``Image`` elements.
``Video`` embeds a movie in a screen.

Styles use CSS-like property names. ``style`` and ``hover`` accept objects or
nested arrays, merged from left to right. Hover styles also apply to keyboard
and gamepad focus. Stable keys retain native layout caches, interaction state,
and animation state across commits. Paint and handler changes skip layout;
text and size changes invalidate affected nodes.

See :doc:`../reference/ui` for components, props, styles, and animation options.
Follow :doc:`accessibility` when naming image buttons, sliders, and text fields.
