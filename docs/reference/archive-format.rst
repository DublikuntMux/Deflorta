.dm archive format
==================

A ``.dm`` archive packages a game's bundled ``main.js`` and assets into one
seekable file. The launcher reads assets directly from the archive, so audio
and video can stream without extracting the game. The engine runtime ships
separately in the launcher.

This reference describes **version 2**, implemented by
``crates/assets/src/archive.rs`` and written by ``crates/cli/src/pack.rs``.
Version 1 archives must be rebuilt with the current CLI.

Create and inspect an archive
--------------------------------

.. code-block:: sh

   deflorta bundle mygame
   deflorta info mygame/build/game.dm
   deflorta run mygame/build/game.dm

``info`` lists archive entries and their decoded and stored sizes. For
compression options and complete player distributions, see
:doc:`../guides/publishing`.

File layout
-----------

All integer fields are unsigned and **little-endian**. Sizes and offsets are
in bytes. Fields and payloads have no alignment padding.

.. kroki::
   :type: graphviz
   :caption: Archive sections in byte order. S is stored_index_size.
   :align: center

   digraph archive {
       graph [bgcolor="transparent", rankdir=TB];
       node [shape=box, style="rounded,filled", fillcolor="#f1f5f9",
             color="#475569", fontcolor="#0f172a", fontname="sans-serif"];
       edge [color="#475569", fontcolor="#475569", fontname="sans-serif"];
       header [label="Header\n24 bytes"];
       index [label="Index\nOne LZ4 frame, S bytes"];
       block0 [label="Block 0 payload\nRaw bytes or one LZ4 frame"];
       block1 [label="Block 1 payload\nRaw bytes or one LZ4 frame"];
       remaining [label="Remaining block payloads"];
       start [shape=plaintext, style="", label="Offset 0"];
       start -> header;
       header -> index [label="Offset 24"];
       index -> block0 [label="Offset 24 + S"];
       block0 -> block1 [label="After block 0's stored bytes"];
       block1 -> remaining;
   }

The index describes every block's stored size and each file's location.
Block payloads follow in index order. They contain either raw bytes or one
complete LZ4 frame; their metadata lives in the index.

Header
------

.. list-table:: Header fields
   :header-rows: 1
   :widths: 12 12 30 46

   * - Offset
     - Length
     - Field
     - Value or meaning
   * - 0
     - 8
     - ``magic``
     - ASCII ``DEFLORTA`` (``44 45 46 4c 4f 52 54 41``).
   * - 8
     - 4
     - ``version``
     - ``u32``, currently ``2``.
   * - 12
     - 4
     - ``block_size``
     - ``u32``, must be ``131072`` (128 KiB).
   * - 16
     - 4
     - ``stored_index_size``
     - ``u32``, length of the index's complete LZ4 frame.
   * - 20
     - 4
     - ``index_size``
     - ``u32``, length of the decoded index.

Both index sizes must be at most 64 MiB (``67108864`` bytes). The stored
index must also fit within the archive after the header.

Decoded index
-------------

Decode the index's LZ4 frame before parsing these fields:

.. code-block:: text

   u32 block_count
   repeat block_count times:
       u32 stored_size
       u8  flags

   u32 entry_count
   repeat entry_count times:
       u16 path_length
       u8  path[path_length]
       u64 size
       u32 first_block

There are no separators or terminators. A block record occupies 5 bytes;
an entry occupies ``14 + path_length`` bytes. The complete decoded index
size is therefore:

.. code-block:: text

   8 + 5 * block_count + sum(14 + entry.path_length)

Blocks
~~~~~~

``stored_size`` is the payload length, including all LZ4 frame overhead for
a compressed block. It must be between 1 and ``131072`` bytes inclusive.

.. list-table:: Block flags
   :header-rows: 1
   :widths: 20 80

   * - ``flags``
     - Payload
   * - ``0x00``
     - Raw file bytes.
   * - ``0x01``
     - One complete LZ4 frame.

All other flag bits are unsupported. Blocks belonging to a file are
consecutive. Each decodes to 128 KiB, except the file's last block, which
contains its remaining bytes. Different files have separate final blocks;
the writer never fills unused space with bytes from the next file.

Entries and paths
~~~~~~~~~~~~~~~~~

``path`` is UTF-8. ``path_length`` counts encoded bytes, with a maximum of
``65535``; it does not count characters. ``size`` is the file's decoded
length. ``first_block`` is a zero-based index into the global block table.
The number of blocks belonging to a file is ``ceil(size / 131072)``.

An empty file has size zero and consumes no blocks. The writer sets its
``first_block`` to the next available block index, which may equal
``block_count`` at the end of the archive.

Paths must be unique, nonempty, normalized paths relative to the game root.
Use ``/`` between components, for example ``images/bg room.png``. Absolute
paths and paths such as ``../outside.png``, ``./main.js``, and
``images/../main.js`` fail validation. The reader checks normalization with
the host platform's path rules; writers should use portable relative paths.

The packer sorts entries by path and writes their blocks in that order.
The reader locates files by their full path rather than entry order.

Compression
-----------

The index is always stored as an LZ4 frame, even if framing increases its
size. Each compressed file block has its own frame and needs no earlier
block to decode. See the official
`LZ4 frame specification <https://github.com/lz4/lz4/blob/dev/doc/lz4_Frame_format.md>`_.

The current writer records decoded content sizes and enables XXHash32
content checksums. Its frames use independent internal LZ4 blocks and
advertise a maximum internal block size of 256 KiB. That frame setting is
separate from the archive's fixed 128 KiB file chunks: the index frame can
contain multiple internal LZ4 blocks.

For file data, the writer keeps a compressed frame only when its complete
size is smaller than the raw chunk. It always stores these extensions raw,
matched without regard to case:

.. code-block:: text

   png jpg jpeg webp gif avif
   mp4 m4a webm mkv ogg oga opus mp3 flac aac
   zip gz woff2

This list controls compression only. Runtime media support is documented in
:doc:`assets`.

Packing uses native LZ4 through the Rust ``lz4`` bindings. The default
compression level is 12; levels 2–12 select HC effort and level 1 selects
fast compression. Compression level is not an archive field and does not
change the decoding procedure. Raw blocks have no content checksum, and the
container has no global checksum, encryption, or signature.

Locate a block and seek within a file
----------------------------------------

Let ``S`` be ``stored_index_size`` and ``B`` be ``131072``. The archive
offset of global block ``k`` is:

.. code-block:: text

   block_offset(k) = 24 + S + sum(blocks[i].stored_size for i < k)

For a decoded file position ``p`` before the end of the file:

.. code-block:: text

   local_block = p // B
   global_block = entry.first_block + local_block
   offset_in_decoded_block = p % B
   decoded_block_size = min(B, entry.size - local_block * B)

Read ``stored_size`` bytes at ``block_offset(global_block)``. Decode them
if the compressed flag is set, then read from ``offset_in_decoded_block``.
For a raw block, the stored and decoded block sizes must match.

For example, a ``131077``-byte file starting at global block 3 uses blocks
3 and 4. Block 3 decodes to ``131072`` bytes and block 4 to 5 bytes. A seek
to position ``131074`` selects global block 4, then byte 2 of its decoded
contents.

The runtime's entry reader caches one decoded block. Seeking changes the
logical position; the next read loads the required block. Seeking past EOF
is allowed and subsequent reads return no bytes. Negative positions or
arithmetic overflow are rejected.

Validation and compatibility
----------------------------

Opening an archive validates the header, index, block table, and entry
ranges. It rejects unsupported versions or block sizes, unknown flags,
invalid stored sizes, payload ranges extending past EOF, invalid UTF-8 or
paths, duplicate paths, and entries extending past the block table.

The decoded index must contain exactly the declared records, with no
trailing bytes. Its LZ4 frame must consume exactly ``stored_index_size``
bytes and produce exactly ``index_size`` bytes. Frame decoding verifies
the footer and any enabled checksum.

File payloads are checked when read. A raw block must have the expected
decoded size; a compressed frame must consume the entire stored payload
and produce exactly the expected decoded size. Consequently, successfully
opening an archive does not establish that every payload is intact.

The writer emits disjoint file block ranges and ends the archive after the
last payload. The current reader permits shared or unreferenced block
ranges and trailing bytes after all indexed payloads. These allowances do
not apply to trailing bytes inside the decoded index or an LZ4 frame.
