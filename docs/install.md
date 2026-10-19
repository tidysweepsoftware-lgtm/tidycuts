# Installing TidyCuts

Download the file for your computer from the
[latest release](../../../releases/latest). Every release has these files:

| File | For |
| --- | --- |
| `TidyCuts-Setup-1.3.6.exe` | Windows 10 and 11, 64-bit. The installer. Best for most people. |
| `TidyCuts-1.3.6-windows-portable.zip` | Windows. No installing: unzip it anywhere and run `TidyCuts.exe`. |
| `TidyCuts-1.3.6-macos.dmg` | Mac with Apple silicon (M1 or newer), macOS 11 or later. |
| `TidyCuts-1.3.6-macos.zip` | The same Mac app in a zip, if the disk image gives you trouble. |
| `TidyCuts-1.3.6-linux-x86_64.AppImage` | Linux, any 64-bit distribution (Ubuntu, Debian, Fedora, Arch, Mint, openSUSE...). |
| `tidycuts_1.3.6_amd64.deb` | Linux: Ubuntu, Debian, Linux Mint and other distributions that use .deb packages. |
| `TidyCuts-1.3.6-linux-x86_64.tar.gz` | Linux, no install: unpack it anywhere and run `./tidycuts`. |
| `SHA256SUMS.txt` | A checksum for every file, to check that a download is the one we built. |

FFmpeg, which TidyCuts uses to read and write video files, is included in every
download. You don't need to install anything else.

Intel Macs aren't supported yet.

---

## Windows

### The installer

1. Run `TidyCuts-Setup-1.3.6.exe`.
2. If Windows shows **"Windows protected your PC"**, choose **More info**, then
   **Run anyway**. (See "Why do I get a warning?" below.)
3. Follow the steps. By default TidyCuts installs just for you and doesn't need an
   administrator password. You can choose a desktop shortcut and whether
   `.tcproj` project files open in TidyCuts.
4. Start TidyCuts from the Start menu.

To remove it: Settings → Apps → TidyCuts → Uninstall. Your projects and
settings are not deleted.

### The portable version

1. Unzip `TidyCuts-1.3.6-windows-portable.zip` to any folder (a USB stick works).
2. Open the `TidyCuts` folder and run `TidyCuts.exe`.

Keep the `ffmpeg` folder next to `TidyCuts.exe`. To remove it, delete the folder.
Your settings are kept in your user profile (`%APPDATA%\TidyCuts`), not in the
folder.

---

## Linux

Pick one of the three files.

- **AppImage** (any distribution): make it executable and run it.
  ```
  chmod +x TidyCuts-1.3.6-linux-x86_64.AppImage
  ./TidyCuts-1.3.6-linux-x86_64.AppImage
  ```
  Some systems need the `libfuse2` package for AppImages (`sudo apt install libfuse2`).
- **.deb** (Ubuntu, Debian, Linux Mint): `sudo apt install ./tidycuts_1.3.6_amd64.deb`, then start
  TidyCuts from the menu or by typing `tidycuts`.
- **.tar.gz** (no install): `tar xzf TidyCuts-1.3.6-linux-x86_64.tar.gz`, then run `TidyCuts/tidycuts`.

Everything TidyCuts needs (FFmpeg, the captions program, the Studio Voice files and the picture
program) is inside; you only need a normal desktop with ALSA or PipeWire sound and, for the
Image Generator, a graphics card with a Vulkan driver (NVIDIA, AMD and Intel all have one).
Linux support is new, so please [report problems](../../issues/new/choose).

---

## Mac

1. Open `TidyCuts-1.3.6-macos.dmg`.
2. Drag **TidyCuts** onto the **Applications** folder in the same window.
3. Open **TidyCuts** from Applications (or Launchpad).

### If macOS blocks TidyCuts

TidyCuts is not yet approved ("notarized") by Apple, because that needs a paid
Apple developer account. So the first time you open it, macOS may stop you. This
is normal for apps from outside the App Store that Apple hasn't checked, and it
only happens once. You'll see one of these messages. Find yours below.

#### "TidyCuts can't be opened because Apple cannot check it for malicious software" or "…cannot verify that it is free of malware"

1. Click **Done** (don't click "Move to Trash").
2. Open the Apple menu → **System Settings** → **Privacy & Security**.
3. Scroll down to the **Security** section. You'll see
   *"TidyCuts" was blocked to protect your Mac.*
4. Click **Open Anyway**, enter your Mac password or use Touch ID, then click
   **Open**.

From now on TidyCuts opens normally. (On macOS 12 and older you can instead
right-click the app, choose **Open**, and click **Open** again.)

#### "TidyCuts is damaged and can't be opened. You should move it to the Trash"

The app isn't damaged. macOS puts a "downloaded from the internet" mark on files,
and sometimes reacts badly to it. Remove the mark:

1. Open the **Terminal** app (Applications → Utilities → Terminal).
2. Paste this line and press Return:

   ```
   xattr -cr /Applications/TidyCuts.app
   ```

3. Open TidyCuts again.

#### The disk image itself won't open

Download `TidyCuts-1.3.6-macos.zip` instead, double-click it to unzip, drag
**TidyCuts** into Applications, and follow the steps above. Or remove the mark from
the disk image first, in Terminal:

```
xattr -d com.apple.quarantine ~/Downloads/TidyCuts-1.3.6-macos.dmg
```

#### Still stuck?

[Open an issue](../../../issues/new/choose) and tell us your macOS version and
what the message says.

To remove TidyCuts, drag it from Applications to the Bin. Your projects and
settings are not deleted.

---

## Why do I get a warning?

Windows and macOS warn about programs whose maker hasn't paid for a signing
certificate (Windows) or an Apple developer account (Mac). It doesn't mean
something is wrong with the file, and it doesn't mean it's safe either: the
warning is only about who published it. Here is what you can check yourself:

- **The code is open.** Everything TidyCuts does is in this repository. It makes
  no network requests of its own and needs no account.
- **The files are built in public.** The installers are built by GitHub's own
  build machines from the code you can read (see the *Actions* tab and
  `.github/workflows/release.yml`).
- **Check the download.** `SHA256SUMS.txt` has the checksum of every file.

  Windows (PowerShell):
  ```
  Get-FileHash .\TidyCuts-Setup-1.3.6.exe -Algorithm SHA256
  ```
  Mac (Terminal):
  ```
  shasum -a 256 ~/Downloads/TidyCuts-1.3.6-macos.dmg
  ```
  The long code that appears should match the line for that file.

We plan to sign the Windows installer and have the Mac app notarized so these
warnings go away.

---

## Where TidyCuts keeps things

| | Windows | Mac |
| --- | --- | --- |
| Settings and autosaves | `%APPDATA%\TidyCuts` | `~/Library/Application Support/TidyCuts` |
| Cache (scrub frames, waveforms, previews) | `%LOCALAPPDATA%\TidyCuts\cache` | `~/Library/Caches/TidyCuts` |

Your projects are saved wherever you choose. The cache can be deleted at any
time (View → Delete Preview Files clears the preview files from inside the app).

## What your computer needs

- **Windows:** 10 or 11, 64-bit, and a graphics chip that supports DirectX 12 or
  Vulkan (most computers made in the last several years).
- **Mac:** Apple silicon (M1 or newer), macOS 11 or later.
- **Linux:** a 64-bit PC, glibc 2.35 or newer (Ubuntu 22.04, Debian 12, Fedora 36 and newer).
- 8 GB of memory is comfortable; 4K video likes more. TidyCuts makes small
  proxy copies of big videos so editing stays smooth.
