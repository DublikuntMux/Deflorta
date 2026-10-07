Build and maintain the documentation
====================================

The handbook is written in reStructuredText under ``docs/`` and built with
Sphinx, the Furo theme, and ``sphinxcontrib-kroki`` for diagrams. Building
documentation does not compile the Rust engine or require game assets to be
decoded.

Install the documentation tools
-------------------------------

Use Python 3.11 or newer. From the repository root:

.. code-block:: sh

   python3 -m venv docs/.venv
   docs/.venv/bin/python -m pip install -r docs/requirements.txt

On Windows, replace ``docs/.venv/bin/python`` with
``docs\.venv\Scripts\python.exe``. If you use uv, the equivalent setup is:

.. code-block:: sh

   uv venv docs/.venv --python 3.12
   uv pip install --python docs/.venv/bin/python -r docs/requirements.txt

The requirements constrain ``setuptools`` below version 82 because
``sphinxcontrib-kroki`` 1.3.0 imports ``pkg_resources``, which
`Setuptools removed in version 82 <https://setuptools.pypa.io/en/stable/deprecated/pkg_resources.html>`_.

Build and browse
----------------

.. code-block:: sh

   docs/.venv/bin/python -m sphinx -b html -n -W --keep-going docs docs/_build/html
   docs/.venv/bin/python -m http.server 8000 --directory docs/_build/html

Open ``http://localhost:8000`` in your browser. HTML output includes navigation,
search, syntax highlighting, and a light/dark theme switch. You can also open
``docs/_build/html/index.html`` directly. Generated output and the virtual
environment are ignored by Git.

The build command checks references and treats warnings as errors. Use it
before submitting a documentation change. To check external links too:

.. code-block:: sh

   docs/.venv/bin/python -m sphinx -b linkcheck -W --keep-going docs docs/_build/linkcheck

Link checking requires network access. See the official
`Sphinx build reference <https://www.sphinx-doc.org/en/master/man/sphinx-build.html>`_
for other output formats and options, and the
`Furo quickstart <https://pradyunsg.me/furo/quickstart/>`_ for theme setup.

Diagrams
--------

Use the ``kroki`` directive for diagrams. The extension renders them as SVG
through ``https://kroki.io`` and caches the results in the build directory.
Rendering new or changed diagrams requires network access; the generated
HTML includes the images and can be viewed offline.

Use transparent backgrounds and neutral colors for Graphviz diagrams. The
documentation stylesheet adapts them to the theme toggle and system color
preference. Check readability in both light and dark modes.

.. code-block:: rst

   .. kroki::
      :type: graphviz
      :caption: Game delivery.
      :align: center

      digraph delivery {
          graph [bgcolor="transparent", rankdir=LR];
          project -> archive -> launcher;
      }

To use your own Kroki server, set ``kroki_url`` in ``docs/conf.py``. See the
`sphinxcontrib-kroki reference <https://github.com/sphinx-contrib/kroki>`_
for supported diagram types and options.

Add or update a page
--------------------

* Put walkthroughs and task-focused help in ``docs/guides/``.
* Put command, API, format, and control lookups in ``docs/reference/``.
* Put contributor-facing engine explanations in ``docs/architecture/``.
* Add new pages to the appropriate toctree in ``docs/index.rst``.
* Use ``:doc:`` links between pages and ``:ref:`` links to named sections.
* Prefer complete runnable examples in tutorials. Explain required assets
  before examples that depend on them.
* Verify behavior against the CLI, runtime declarations, and source code.
  Keep examples and defaults in step with the implementation.

``README.md`` remains a short repository entry point. ``DESIGN.md`` links to
the architecture pages. Maintain the handbook as the source of detailed
documentation rather than duplicating it in those files.

The official
`Sphinx getting-started guide <https://www.sphinx-doc.org/en/master/usage/quickstart.html>`_
explains reStructuredText, toctrees, and cross-references.
