# Making a release

Normal pushes only run the Linux tests and a Windows build, because the Mac
build costs ten times as much to run. The installers are built by a separate
workflow, `.github/workflows/release.yml`.

1. Check that `main` is green (the CI badge or the Actions tab).
2. Change `version` in the root `Cargo.toml`, and move the entries in
   `CHANGELOG.md` under the new version.
3. Commit and push to `main`.

4. Start the **Installers** workflow: Actions → Installers → Run workflow, platform
   *both*, and tick *Also make a draft release*. It builds:
   - `TidyCuts-Setup-<version>.exe`, the Windows installer (Inno Setup);
   - `TidyCuts-<version>-windows-portable.zip`, the same program in a folder
     that runs from anywhere, with FFmpeg included;
   - `TidyCuts-<version>-macos.dmg`, the Mac disk image (Apple Silicon, with an
     LGPL FFmpeg when its build succeeds; the app works without it if FFmpeg is
     installed), and `TidyCuts-<version>-macos.zip`, the same app as a zip.
5. It then creates a **draft** release with the files, a `SHA256SUMS.txt` and the
   notes from `.github/release-notes.md` attached. Read the notes, try the
   installers on a real machine, and press Publish.

Leave "Also make a draft release" unticked to just try the installers. Publishing the
draft creates the version tag (for example `v1.0.0`) on the commit that was built.

## Before each public release

- Run `cargo deny check` (see `deny.toml`) and read the licence list again.
- Check the version in the root `Cargo.toml` (it names every file) and the
  download links in `README.md` and `docs/install.md`.
- The repository's **Security** tab should have private vulnerability reporting
  switched on (Settings > Code security).

## Signing (not done yet)

The installers are not signed, so Windows SmartScreen and macOS Gatekeeper warn
the first time. `docs/install.md` explains how to get past that. To remove the
warnings: a Windows code-signing certificate for the installer, and an Apple
Developer account to sign and notarize the Mac app.
