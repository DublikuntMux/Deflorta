Native UI, rendering, and media
===============================

The JavaScript side supplies native descriptors; Rust retains the displayed
tree and owns per-frame work. See :doc:`runtime` for descriptor and callback
handling, and :doc:`../reference/ui` for the game-facing props.

Layout and text
---------------

Taffy provides CSS flexbox and grid layout: positions, insets, sizes, padding,
margin, gap, flex direction/wrap/grow/shrink, alignment, equal grid tracks, and
overflow. Cosmic-text measures text; words are not broken as an overflow
fallback, following CSS ``overflow-wrap: normal`` behavior.

Games lay out at a virtual resolution such as 1280×720. The engine scales
and letterboxes the result, shaping text at physical size to keep it crisp.
Fonts come from the game to keep rendering consistent across machines.
Inherited text properties include color, size, family, weight, italic,
line height, alignment, and shadow. Rich text is supplied as spans;
decoration and ruby annotations use the shaped glyph layout.

Unchanged text retains its shaped buffer and measured size without copying
spans or traversing glyph runs. Shadow and ruby buffers share their owner's
lifetime, including elements identified by child index. Surface scale changes
invalidate text measurements.

Node identity and incremental updates
-------------------------------------

A node's identity derives from its parent and stable key, or its child index.
Keys preserve layout, animation, focus, and typewriter state across commits
and reorders. Style/content changes invalidate affected layout caches;
unrelated subtrees retain theirs. Paint-only changes such as handlers, color,
and opacity skip layout. Exit ghosts use frozen rectangles outside the
layout tree.

Typewriter and animations
-------------------------

Spans carry timed waits, click-waits, and fast-forward points. A click while
typing reveals text up to the next click-wait; a click at that wait resumes
typing. The current line is re-shaped when its visible length changes.

``enter`` animates from supplied values when a keyed node appears. A removed
node with ``exit`` becomes a ghost at its old z-position with frozen layout
until the animation finishes. The scene runtime uses this for transitions;
the outgoing background remains opaque beneath the incoming one for a
crossfade. ``move`` animates keyed nodes from previous layout positions.

ATL programs with ``set``, tweens, easing, ``pause``, ``parallel``, and
``repeat`` are interpreted in Rust each frame. They restart only when the
program changes. Similarity transforms compose scale, rotation, and
translation down the tree.

Transition masks reveal through an image (dark first), a wipe, or pixellation
in screen space in the quad shader. Exiting elements use the inverted mask.

Input and tooltips
------------------

Rust applies hover styles without a JavaScript round trip. Clicks bubble from
the topmost element to the nearest handler. Arrow keys and gamepads move
focus spatially to the nearest focusable element in the pressed direction;
focused elements use hover styles.

Sliders drag/step and text fields edit natively, including IME commits, then
report changes to handlers. Scroll containers clip children and scroll with
wheel and focus. Tooltips track hover/focus.

The default tooltip binds a text node with ``tooltipText``. Rust updates its
text and visibility directly, so hovering save slots does not re-render
screens or commit another tree. Custom screens calling ``tooltip()`` still
invalidate when its value changes; unchanged values are ignored.

Graphics pipeline
-----------------

One instanced wgpu pipeline draws rectangles and images. Signed-distance
rounded boxes provide antialiased corners and borders, including rounded
images. Consecutive quads with the same texture are batched.

Glyphon draws text. The draw list splits into layers when a shape follows
text to preserve painter's order with few draw calls. Blending uses sRGB
space, non-sRGB targets, and glyphon's ``ColorMode::Web`` to match CSS-like
translucency. Overflow and letterbox clipping use per-batch scissors.

Adapters request ``LowPower`` and downlevel limits for integrated GPUs and
GL-capable hardware. Performance depends on the workload and backend; the
event-driven loop avoids continuous redraws while idle.

Asset decoding and video
------------------------

Images decode on a background pool. An incoming tree waits up to 1.5 seconds
for image decoding before display, so transitions can begin with their images.
Workers wake the window event loop after publishing a result; loading does
not require periodic polling. Explicit deadlines preserve the image wait
limit and thumbnail settling timeout. Connected gamepads still poll for input.
``preload()`` decodes early. Unused textures expire after 60 seconds;
see :doc:`../guides/debugging` for cleanup and forced-unload behavior.

Video uses a background thread decoding a few frames ahead, paced by the
wall clock, and uploads frames into a reused texture. MP4 AAC and WebM Vorbis
soundtracks play through kira. Format limits are in
:doc:`../reference/assets`. Both directories and archives provide seekable
media reads through the shared file abstraction.

Sound effects decode on an audio-loading worker. Music, voice, and video
soundtracks are opened and prepared there too, including Kira's initial
decoder seek. Playback handles and track controls stay on the event thread.
Stopping or replacing a pending stream invalidates its result, and video
soundtracks start at the video's elapsed position after loading.

Save thumbnails and headless rendering
--------------------------------------

Opening the game menu or quick-saving requests an offscreen render of the
current tree before the menu appears. The capture is downscaled and written
beside the save when saving occurs.
PNG encoding, atomic writes, and deletion run on the same ordered storage
worker as JSON saves. Completion invalidates cached thumbnail textures and
releases image requests waiting for their file to be ready. Failed writes
also release these waits. Shutdown drains queued saves and thumbnails before
returning; Android suspension flushes them before yielding to the OS.

The headless front end uses the same engine with an offscreen renderer and
scripted input. It captures screenshots without a window; see
:doc:`../guides/testing`. Developer-console UI is omitted from these renders
and thumbnails.
