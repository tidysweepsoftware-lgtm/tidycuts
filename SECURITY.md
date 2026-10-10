# Security

## Reporting a problem

If you find a way to make TidyCuts do something harmful (run a program you
didn't ask for, read files it shouldn't, let another program on the network take
control of the editor, crash on a crafted file in a way that could be abused),
please **don't open a public issue**. Report it privately instead: open this
repository's **Security** tab, choose **Report a vulnerability**, and say what you
found and how to repeat it. Only the maintainers can see the report. You'll get an
answer within a few days, and we'll tell you when it's fixed.

## What TidyCuts does with your computer

- **The assistant connection is off by default.** When you turn it on in
  Preferences, TidyCuts listens on `127.0.0.1` only, so nothing outside your
  computer can reach it. Every request must carry a long random key, which you
  can replace at any time. Requests from web pages are refused.
  Assistants can edit the open project and export files; they can't run other
  programs.
- **Project files** are plain JSON. TidyCuts never runs anything stored in
  them.
- **FFmpeg** is run as a separate program to read and write media files. It is
  found next to TidyCuts, in `TIDYCUTS_FFMPEG_DIR`, or on your PATH.
- TidyCuts makes **no network requests** of its own.

## Supported versions

Only the latest release gets security fixes.
