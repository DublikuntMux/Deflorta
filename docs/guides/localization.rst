Translate a game
================

Translations are JSON tables in ``tl/<language>.json``. Each key is source
text and each value is a translation or ``null``. A null or missing translation
displays the source text.

Extract and update text
-----------------------

.. code-block:: sh

   deflorta translate update uk -p mygame
   deflorta translate status -p mygame
   deflorta translate missing uk -p mygame

``update`` extracts dialogue, character names, menu prompts and choices, input
questions, game titles, explicit ``_()`` strings, and engine interface strings.
It preserves existing translations and adds new entries as ``null``.
Without language arguments, it updates all existing tables. Use ``--prune``
to remove entries for text no longer present in the game.

Edit ``mygame/tl/uk.json``, for example:

.. code-block:: json

   {
     "Start": "Почати",
     "Welcome to your first story.": "Ласкаво просимо до вашої першої історії."
   }

Run ``deflorta check mygame`` to validate the tables. Values must be strings
or null; duplicate keys are errors.

Offer languages in preferences
------------------------------

.. code-block:: javascript

   import { configure } from "deflorta";

   configure({
     languages: [
       { id: null, name: "English" },
       { id: "uk", name: "Українська" },
     ],
   });

``null`` selects the language the story is written in. You can also switch
programmatically with ``setLanguage("uk")`` or ``setLanguage(null)``.
``translations(language, table)`` registers translations directly in code.

Translate custom interfaces
---------------------------

Wrap custom UI strings and accessible names in ``_()``:

.. code-block:: jsx

   import { _, Text } from "deflorta";

   function Title() {
     return <Text>{_("Your journey")}</Text>;
   }

The extractor reports template strings with substitutions because their final
text cannot be determined statically. Use separate lines or explicit ``_()``
pieces where appropriate. Check the resulting text in each language, including
buttons, layout, fonts, and screen-reader labels.
