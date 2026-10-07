# Deflorta

A visual novel engine written in Rust. Write stories, characters, menus, and
screens in JavaScript and JSX. Desktop runtimes support Linux, Windows, and
macOS; Android games export through reusable Gradle templates.

## Start here

The [documentation handbook](docs/index.rst) contains setup instructions,
game-authoring guides, API references, and engine design notes.

- [Build the engine and make your first game](docs/getting-started.rst)
- [Write stories](docs/guides/writing-stories.rst) and [customize screens](docs/guides/custom-screens.rst)
- [Translate](docs/guides/localization.rst), [publish](docs/guides/publishing.rst), or [export to Android](docs/guides/android.rst)
- [CLI reference](docs/reference/cli.rst), [scripting API](docs/reference/scripting.rst), and [UI reference](docs/reference/ui.rst)
- [Engine architecture](docs/architecture/index.rst)

## Quick start

Install Rust through rustup, Python 3.9+, clang/libclang, and on Linux the
Speech Dispatcher development library (such as `libspeechd-dev`). The repository
selects nightly Rust. From the repository root:

```sh
python3 scripts/build-dist.py
dist/deflorta run game
dist/deflorta create mygame --title "My Game"
dist/deflorta check mygame
dist/deflorta run mygame
dist/deflorta publish mygame
```

On Windows, use `dist/deflorta.exe`. Keep the complete `dist/` directory together;
the CLI needs its adjacent runtimes and templates. `game/main.js` is the full demo.

## Build the documentation

With Python 3.11+:

```sh
python3 -m venv docs/.venv
docs/.venv/bin/python -m pip install -r docs/requirements.txt
docs/.venv/bin/python -m sphinx -b html -n -W --keep-going docs docs/_build/html
```

Open `docs/_build/html/index.html`, or follow the
[documentation build guide](docs/documentation.rst) for serving, Windows commands,
and link checks. The site uses Sphinx with the Furo theme.

## License

Engine: [MIT](LICENSE.md). Demo fonts: Noto Sans
([font license](game/fonts/LICENSE-noto.txt)).
