Export an Android game
======================

Android exports use a reusable Gradle template and prebuilt native engine
libraries. There are two separate jobs: engine developers build the templates;
game authors export APKs or Android App Bundles using those templates.

Build Android engine templates
------------------------------

Start with the desktop prerequisites in :doc:`../getting-started`. Install
``cargo-ndk``, the Rust targets, and Android NDK r28 or newer. Set
``ANDROID_NDK_HOME`` to the installed NDK directory.

.. code-block:: sh

   cargo install cargo-ndk --locked
   rustup target add aarch64-linux-android x86_64-linux-android
   export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/28.2.13676358"
   python3 scripts/build-dist.py --android

The default ABI is arm64-v8a, named ``android-aarch64``. To include x86_64
for emulators as well:

.. code-block:: sh

   python3 scripts/build-dist.py --android --android-abi arm64-v8a --android-abi x86_64

Both profiles contain native libraries under
``target/<platform>/<debug|release>/jniLibs/``. Android debug exports use an
unoptimized engine without the desktop developer console. The script also
builds the host CLI and launchers needed to check games.

When no prebuilt SpiderMonkey archive exists, native compilation needs its
source-build tools: Python, make, clang, and libclang. The script sets the
NDK path and minimum Android API to match cargo-ndk.

Prepare the export tools
------------------------

Game export requires no Rust toolchain or NDK when the engine templates are
already available. Install Java 17 or newer and the Android SDK, set
``ANDROID_HOME``, and install SDK platform 36 and build-tools 36.0.0.
The included wrapper downloads Gradle 9.4.1; the template uses Android Gradle
Plugin 9.2.1 and Kotlin DSL.

Create a debug APK
------------------

From the Deflorta repository, using its demo project:

.. code-block:: sh

   dist/deflorta publish game --platform android-aarch64 --debug \
     --android-package com.example.mygame
   adb install -r game/dist/android-aarch64/*.apk

Replace ``game`` with your project's path for your own game. Publishing checks
and bundles the game with the host runtime, then packages the archive in an
APK. ``--name`` chooses the artifact name.

Choose a stable reverse-domain application id with ``--android-package``.
The default is ``org.deflorta.game_<game id>``, with hyphens replaced by
underscores. The version name comes from ``configure({ version })`` or defaults
to ``1.0``. ``--android-version-code`` defaults to 1; increase it for updates.

Create a signed release
-----------------------

Release exports require all four signing environment variables:

.. code-block:: sh

   export DEFLORTA_KEYSTORE=/absolute/path/to/release.jks
   export DEFLORTA_KEYSTORE_PASSWORD='your-keystore-password'
   export DEFLORTA_KEY_ALIAS='your-key-alias'
   export DEFLORTA_KEY_PASSWORD='your-key-password'
   dist/deflorta publish game --platform android-aarch64 \
     --android-package com.example.mygame --android-format aab --android-version-code 2

``--android-format aab`` creates an Android App Bundle; the default is ``apk``.
Signing passwords are not written to the generated Gradle project.

The output's ``android/`` directory retains that project for inspection or
custom builds. A subsequent export regenerates it. Keep reusable template
changes in the engine distribution's ``template/android/`` instead.

Runtime behavior
----------------

Games require Android 8.0/API 26 or newer, open in landscape, accept touch and
GameActivity keyboard input, and recreate the GPU surface after backgrounding.
Saves live in the app's private files directory. The packaged ``game.dm`` is
copied there at startup to keep large media seekable.
