# Letting an AI assistant edit with you

TidyCuts has a small server inside it that lets an AI assistant look at your
project and edit it while you watch: add and cut clips, change settings, add
titles, line clips up by sound, remove silences, check the picture and export.
Every change can be undone with Ctrl+Z.

It is **off until you turn it on**: Preferences > AI assistants.

## How it stays safe

- It only listens on `127.0.0.1`, your own computer, so nothing else on your
  network can reach it.
- Every request needs the long random key shown in Preferences. **Make a new
  secret key** replaces it and cuts off anything using the old one.
- Web pages can't use it.
- Assistants can edit the open project and export files. They can't run other
  programs on your computer.

## Claude

Claude Code, in a terminal: copy the command from Preferences and run it once.
It looks like

```
claude mcp add --transport http tidycuts http://127.0.0.1:47861/mcp --header "Authorization: Bearer <your key>"
```

Then ask Claude to edit: "remove the silences from the clip on the timeline and
add a title that says Welcome".

## OpenAI Codex

Codex also runs on your computer. Add the lines Preferences shows under
"Connect ChatGPT's Codex" to its settings file, `.codex/config.toml` in your user
folder, and start Codex again:

```
[mcp_servers.tidycuts]
url = "http://127.0.0.1:47861/mcp"
http_headers = { Authorization = "Bearer <your key>" }
```

## ChatGPT website and phone app

These can't connect. ChatGPT only talks to servers on the public internet, and
TidyCuts' server deliberately stays on your computer. Making it reachable from
the internet would mean opening your editor to everyone, so it isn't offered.

## What an assistant can do

It gets 37 tools, listed with their descriptions when it connects:
`get_project`, `import_media`, `add_to_timeline`, `split_clip`, `delete_clips`,
`trim_clip`, `move_clip`, `update_clip` (any setting), `apply_effect`,
`set_transition`, `set_speed`, `add_title`, `add_layout`, `add_visualizer`,
`set_mix`, `remove_silence`, `split_at_scenes`, `add_beat_markers`,
`sync_by_audio`, `make_proxy`, `add_voiceover`, `make_thumbnail`, `render_frame` (a picture of the result),
`export_video`, `undo` and more. Any MCP-capable assistant that supports a
local HTTP server with a bearer key can use them.
