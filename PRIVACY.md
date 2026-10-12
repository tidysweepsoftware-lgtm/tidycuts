# Privacy policy

*TidyCuts 1.1.0, from TidySweep Software. Last updated 12 October 2026.*

**Short version: TidyCuts doesn't collect, send or share any information about
you. It has no account, no ads and no tracking.**

## What TidyCuts does with your information

- **It collects nothing.** TidyCuts has no analytics, no crash reporting that
  sends anything out, and no sign-in. We never receive your videos, your
  projects, your name, your email address or anything about your computer.
- **It makes no network requests of its own.** It works fully offline. It doesn't
  check for updates by itself and doesn't download anything.
- **Your files stay where you put them.** TidyCuts reads the videos, pictures and
  sounds you choose, and writes the projects and exports you ask for, in the
  places you choose. It also keeps a few helper files on your computer: settings
  and autosaved copies of your project, and a cache of small preview pictures,
  waveforms and proxy copies of big videos. You can delete them at any time. See
  [docs/install.md](docs/install.md) for where they are.
- **FFmpeg**, which is included, runs on your computer as a separate program to
  read and write video files. It doesn't connect to anything either.

## The optional assistant connection

TidyCuts has a feature, **off by default**, that lets an AI assistant you already
use (such as Claude Code) edit the open project alongside you. If you turn it on
in Preferences, TidyCuts listens for requests on `127.0.0.1`, your own computer
only, and every request must carry a secret key that you can replace at any time.
Nothing outside your computer can reach it. What the assistant does with the
project you share with it is governed by that assistant's own terms and privacy
policy, not by TidyCuts.

## Children

TidyCuts doesn't collect information from anyone, including children.

## Changes

If this ever changes, the change will be shown here and in the
[changelog](CHANGELOG.md) before it reaches you.

## Contact

Questions about privacy: write to tidysweepsoftware@gmail.com or [open an issue](../../issues/new/choose) on this page.
For anything sensitive, use the private **Report a vulnerability** form on the
Security tab (see [SECURITY.md](SECURITY.md)).
