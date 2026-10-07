# Deflorta engine design

The design documentation lives in the [Sphinx handbook](docs/index.rst).
JavaScript describes stories and screens; the native Rust core handles layout,
animation, input, rendering, media, and accessibility.

- [Architecture and event model](docs/architecture/index.rst): goals, subsystems,
  platform status, and future work.
- [JavaScript runtime and replay](docs/architecture/runtime.rst): the native
  boundary, callback lifetimes, embedded modules, saves, and rollback.
- [Native UI, rendering, and media](docs/architecture/rendering.rst): retained
  layout, text, animation, graphics, decoding, and thumbnails.
- [Workspace, distributions, and archives](docs/architecture/distribution.rst):
  the five crates, launcher/template layout, bundling, and the version 2 format.

For game development, start with [your first game](docs/getting-started.rst).
See [Build and maintain the documentation](docs/documentation.rst) to generate
the HTML handbook locally.
