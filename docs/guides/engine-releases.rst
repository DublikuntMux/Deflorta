Automated engine releases
==========================

The ``Release engine and documentation`` GitHub Actions workflow builds and
publishes a release when the latest commit pushed to the repository's default
branch has an exact title such as ``v1.0.0``. Other commit titles skip the
release jobs. Version titles use ``vMAJOR.MINOR.PATCH`` with no leading zeroes,
suffixes, or additional text. A version commit must be the last commit in the
push. A tag push alone does not trigger this workflow.

Repository setup
-----------------

In **Settings → Pages → Build and deployment**, choose **GitHub Actions** as
the source. If the ``github-pages`` environment requires approval, deployments
wait for that approval; remove its required reviewers for fully automatic
publishing. The workflow grants its release job ``contents: write`` and its
Pages job ``pages: write`` and ``id-token: write``. Repository or organization
policies must allow those permissions and the actions used by the workflow.
No personal access token or production Android signing secrets are needed.

Create a release
-----------------

After committing the changes you want to ship, create a version commit and
push the default branch:

.. code-block:: sh

   git commit --allow-empty -m "v1.0.0"
   git push origin HEAD

The workflow resolves the repository's nightly Rust toolchain once for both
build hosts. It stamps the workspace and lockfile versions inside the runners,
so ``deflorta --version`` reports ``1.0.0``. It does not commit version changes
back to the source branch.

Linux and Windows builds create debug and release launchers. The Linux builder
also creates Android arm64-v8a and x86_64 native libraries with NDK r28, then
checks APK and signed AAB exports for both ABIs. Signing uses a temporary test
key that is never included in release assets. Desktop exports are checked on
both hosts, and Linux-to-Windows publishing is checked after assembly.

After the builds, export checks, and strict Sphinx build pass, the workflow
creates the version tag at the triggering commit and publishes the GitHub
release. Its changelog lists commits since the closest preceding version tag
in the commit history; the first release includes the complete history.
Version-marker commits are omitted. Documentation from the same commit is
then deployed to GitHub Pages.

Release downloads
------------------

For ``v1.0.0``, the release contains:

* ``deflorta-v1.0.0-linux-x86_64.tar.gz``: Linux CLI and all export runtimes.
* ``deflorta-v1.0.0-windows-x86_64.zip``: Windows CLI and all export runtimes,
  including the MSVC runtime DLLs needed by the desktop executables.
* ``deflorta-v1.0.0-android-export.tar.gz``: Android native runtimes and Gradle
  template, for adding Android exports to another engine distribution.
* ``SHA256SUMS``: SHA-256 checksums for the three archives.

Every archive has a top-level ``dist/`` folder and includes ``VERSION`` and
the engine license. Both desktop distributions include game templates and
``target/linux-x86_64``, ``target/windows-x86_64``, ``target/android-aarch64``,
and ``target/android-x86_64``. Keep the complete distribution together.
Extract the Android archive over an existing distribution to install its
``target/`` and ``template/android/`` folders.

Linux executables are built on Ubuntu 24.04 and require compatible system
libraries, including Speech Dispatcher, ALSA, and udev. On Ubuntu 24.04,
install ``libspeechd2``, ``libasound2t64``, and ``libudev1``. Game authors using
Android exports also need Java 17, SDK platform 36, and build-tools 36.0.0;
follow :doc:`android` for their own release signing setup. Rust and the NDK
are not needed to export games from downloaded distributions.

Retry a failed run
------------------

Use **Actions → Release engine and documentation → Re-run jobs** to retry the
same commit. **Run workflow** also works while the default branch's latest
commit still has the version title. An existing release at that same commit
has its assets and notes updated. Reusing a version whose tag points to a
different commit fails instead of replacing the old version.

The engine release remains published if only the later Pages deployment
fails; retry the failed job after correcting the Pages settings.
