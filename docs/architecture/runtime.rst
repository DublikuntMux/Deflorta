JavaScript runtime and replay
=============================

This page describes engine internals. For game code, use
:doc:`../guides/writing-stories` and :doc:`../reference/scripting`.

The JavaScript–Rust boundary
----------------------------

The boundary uses direct runtime values, grouped native modules, and explicit
UI commits. It follows ideas from React Native's
`New Architecture <https://reactnative.dev/architecture/landing-page>`_,
but Deflorta uses SpiderMonkey's JSAPI and does not depend on React Native
or JSI.

``script/mod.rs`` owns the runtime, module loader, and registered entry points.
``script/native.rs`` defines typed command/event schemas and native functions.
``script/value.rs`` reads serde types directly from rooted JavaScript values
and creates event objects without JSON text. Struct fields match pinned
property atoms; unknown fields are skipped without reading their values.

.. list-table:: Native modules
   :header-rows: 1
   :widths: 25 75

   * - Module
     - Responsibility
   * - ``log``, ``connect``
     - Logging and registration of dispatch/flush callbacks.
   * - ``files``
     - Read text files inside the game root.
   * - ``storage``
     - Atomic per-game saves/preferences and a direct array of stored entries.
   * - ``timers``
     - Set and clear engine timers.
   * - ``app``
     - Platform query, configuration, fullscreen, self-voicing, and quit.
   * - ``audio``
     - Music, sound, voice, and channel volumes.
   * - ``ui``
     - Tree commits, reveal, preloading, and save thumbnails.

Rust-to-JavaScript events include ``boot``, ``click``, ``handler``, ``key``,
``wheel``, ``tooltip``, ``timer``, ``revealed``, and ``quit``.
Background storage failures arrive as ``storageError`` and the core runtime
reports them through the regular ``error`` listeners.
Click events include a handler, button, and reveal state; handler events
carry widget values. Key events carry key/down/repeat/modifiers and reveal
state. The :download:`type declarations <../../crates/js-bridge/runtime/deflorta.d.ts>`
describe their payloads.

Native calls read arguments synchronously. Mutations are queued as Rust enum
values and applied after JavaScript returns, avoiding reentrant borrowing of
the engine. Direct arguments are plain data: own enumerable properties,
``undefined`` omitted, non-finite numbers treated as null, and no invocation
of ``toJSON``. JSON remains the on-disk save and story snapshot format.

Storage loads a session snapshot on its worker during configuration, before
boot. Reads and listings use that snapshot; writes and removals update it
immediately and queue ordered, atomic disk operations. Failed operations
restore the latest persisted value unless a newer operation is pending.
Errors are logged even during shutdown, when no event loop remains to
deliver them. External edits become visible in the next session.

UI descriptors and callback lifetimes
-------------------------------------

Native descriptors are nested elements with kinds ``box``, ``text``, ``image``,
``slider``, ``input``, or ``video``. They carry keys, styles, hover overrides,
handlers, accessibility props, children, text/spans, sources, and animations.
Rust reads the tree in place.

Callbacks stay in a rooted JavaScript array per commit. Rust holds a generation
and index, and events resolve these handles to functions. The displayed tree's
callbacks stay alive while a replacement waits for images or thumbnail capture.
Older arrays are released when the replacement is shown.

Commits read the full tree. Native nodes reconcile by identity and kind,
retaining taffy layout and measurement caches. Changes to layout styles,
children, or intrinsic measurements dirty affected nodes and ancestors.
Paint and handler changes skip layout. This is not Fabric's immutable
shadow-node sharing or a concurrent renderer.

Embedded runtime modules
------------------------

Games import ES modules. The ``deflorta-script-build`` crate lowers JSX in
``.js`` and ``.jsx`` through ``deflorta/jsx-runtime``. The CLI compiles game
scripts before running, checking startup, or bundling them. The runtime build
uses the same compiler for embedded modules. Launchers load JavaScript and
have no JSX compiler or Oxc runtime dependency. Release builds minify
runtime JavaScript with Oxc before embedding it. ``build.rs`` also compiles
``src/render/*.wgsl`` to SPIR-V with naga.

.. list-table:: Modules in ``crates/js-bridge/runtime/``
   :header-rows: 1
   :widths: 30 70

   * - Module
     - Responsibility
   * - ``deflorta/core``
     - Native modules, timers, config, storage, event bus, error reporting.
   * - ``deflorta/notifications``
     - Notification handles, message/state updates, duration, and dismissal.
   * - ``deflorta/text``
     - Text-tag parsing and translations.
   * - ``deflorta/components``
     - JSX elements, keyed component reconciliation, hooks, effects.
   * - ``deflorta/jsx-runtime``
     - Automatic JSX helpers and fragments.
   * - ``deflorta/ui``
     - Native components, theme, screen stack, tooltips, commits.
   * - ``deflorta/scene``
     - Images, positions, transitions, ATL, scene state, music/sound.
   * - ``deflorta/story``
     - Labels and interactions, history, store, persistence, seen text,
       rollback, saves, input bindings, preferences.
   * - ``deflorta/screens``
     - Dialogue/NVL, quick menu, choices, input, movie, history, main/game
       menus, saves, preferences, confirmation, tooltips, errors.
   * - ``deflorta``
     - Public re-exports.

The component renderer resolves functions and fragments into native
descriptors before a commit. Component type, parent, and key determine hook
identity. Setters batch updates until flush; effects run after commit, with
cleanup on dependency changes or unmount. Setters on unmounted components do
nothing. A temporarily hidden interface retains mounted screens without
rendering them. Hook state remains local; saved story state lives in ``store``.

.. _replay-model:

Saves and rollback through replay
---------------------------------

JavaScript closures and suspended async functions cannot be serialized.
Deflorta saves inputs and snapshots instead of stacks:

1. Each interaction, such as ``say``, ``menu``, ``pause``, or a custom
   ``checkpoint``, has a numbered checkpoint.
2. Entering a label through ``jump`` starts a root. The runtime snapshots
   ``store``, background/sprites/music, shown game screens, and the RNG.
3. A save records the root label/snapshot, checkpoint inputs, target checkpoint,
   and metadata such as checkpoint kinds and history.
4. Loading restores the snapshot and re-runs the label. Checkpoints before
   the target resolve with recorded inputs, sound effects are suppressed,
   and the first commit is shown without transitions.
5. Rollback performs the same replay to an earlier checkpoint. Earlier roots
   are retained in memory, 64 by default, so rollback can cross jumps.

Story code must use serializable ``store`` state, seeded engine randomness,
and engine awaits. See :doc:`../guides/writing-stories` for the author rules.
During load, incompatible checkpoint sequences or replay failures trigger
scene restart recovery. A missing label or unsupported save format still
prevents loading; changing ``config.version`` alone does not validate replay.
