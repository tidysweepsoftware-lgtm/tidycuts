# How TidyCuts was made

**TidyCuts was written by an AI.** The code, the tests, the built-in titles,
effects, filters, stickers and music, and the documentation were all written by
[Claude](https://www.anthropic.com/claude), an AI model made by Anthropic, working
in [Claude Code](https://www.anthropic.com/claude-code), Anthropic's coding tool.
The person behind TidySweep Software decided what to build, said what was wrong
and what looked bad, supplied the photos, and tested every version on real
Windows and Mac computers.

We say this up front because you should know what you are installing, and because
we think it's a good story: a complete desktop video editor, about 75,000 lines of
Rust plus its graphics-card shaders, made through conversation.

## How the work went

1. **Plain-words requests.** The owner described what they wanted the way a video
   maker would: "the timeline should close the gap when I cut", "the effects look
   20 years old", "I want a button to find silences", "it crashes when I minimize
   it during export". There were no specifications and no tickets.
2. **Claude planned and wrote the code.** For each request Claude read the existing
   code, made a plan, and wrote the change in Rust, with tests.
3. **Automatic checks.** Every change has to pass the same checks you can run
   yourself (`scripts/check.sh`): the code is formatted, a strict linter treats
   every warning as an error, and more than 300 automated tests run. GitHub runs
   them again on every push, on Linux, and builds the Windows version.
4. **Looking at the result.** Claude doesn't just trust that code works. It drew
   contact sheets of every title, effect, filter, transition and sticker and
   looked at them, and ran the real app on a virtual screen and looked at that
   too. (Several bugs were found that way, for example effects that rendered as
   a black picture on some graphics drivers.)
5. **Testing on real machines.** The owner installed the Windows and Mac builds on
   their own computers, tried them, and reported what was wrong, often with a
   screenshot. Those reports became the next round of fixes.

## What is built from scratch

- **All the built-in creative content is made by TidyCuts itself.** Titles are
  drawn from fonts; transitions, effects and filters are small graphics-card
  programs (shaders); stickers are drawn with vector shapes in code; the music
  and sound effects are synthesised in code. None of it is stock footage or
  someone else's presets, so all of it is free to use in your videos.
- **The sample pictures** shown on library tiles are photographs taken by the
  project's owner, and a cut-out portrait of them.
- **Ideas, not assets.** To find out what looks current, Claude read public
  articles and free-element pages made by other creators. Nothing was copied from
  them: no files, no designs, no names. Everything was built again from nothing.
- **Fonts, icons and libraries** come from open projects under their own licences.
  See [THIRD-PARTY-NOTICES.md](../THIRD-PARTY-NOTICES.md).

## What this means for you

We would rather be straight about the limits than hope you don't ask:

- **It can have bugs, like any software.** AI-written code is not magic. The test
  suite is large, but real videos on real computers are where surprises live.
- **Some things can't be tested everywhere.** There are many graphics cards. The
  graphics-card video encoders (NVIDIA, AMD, Intel, Apple) can't be tried on all
  of them. If one misbehaves on yours, switch off GPU acceleration in the Export
  window and tell us.
- **No outside security audit yet.** The program makes no network requests, has no
  account and no tracking, and keeps the assistant connection off unless you turn
  it on (see [SECURITY.md](../SECURITY.md)). But nobody independent has audited
  it. Because the code is open, anyone can.
- **Review is welcome.** If you read the code and see something wrong, odd or
  unsafe, please open an issue. A person reading AI-written code is exactly the
  second pair of eyes it needs.

## Why it's open source

Because trust shouldn't depend on a company's say-so. You can read every line,
build it yourself, and see that it does only what it claims. TidyCuts is released
under the [Apache License 2.0](../LICENSE): free to use, change and share, for any
purpose.

## Working with an AI on this project

If you contribute, you're welcome to use AI tools too, as long as the change passes
`scripts/check.sh` and you've read and understood what you're sending. See
[CONTRIBUTING.md](../CONTRIBUTING.md). Commits made with Claude's help say so in
their message.

TidyCuts also has an optional connection that lets an AI assistant you already
use edit your project alongside you. It's off by default; see
[assistants.md](assistants.md).
