# Chat settings live in the app's data folder, and the state folder is held whole

**Amends [ADR 0067](../0067-a-chat-runs-in-a-sandbox-charter-compiles-for-its-harness.md), 2026-10-28.**

## Chat settings files

Some Claude Code chats have settings too long for one word of a command line. Purlis writes those settings to a file and starts the chat with `--settings <file>`.

That file now lives in the app's data folder, beside the skill sets: `<app data>/chat-settings/<clone key>/<chat id>.<digest>.json`. That folder is outside every plane, so outside what a sandboxed chat writes unless a person's grant reaches it. Its move into the data home's `app-state`, which every sandbox holds as an integrity path, follows (#3718). The file's name carries the digest of what purlis wrote. Purlis reads a file back only while it still holds that; a file so named that holds anything else is read as sandboxed.

Earlier builds kept these files in the plane's state, under `chat-settings/`. That folder is an integrity path now. It is swept, and never read.

## The state folder is held whole

The state folder that a project has (`.purlis/` or `.charter/`) is held as an entry in purlis's own wrap (Codex and opencode on macOS): the chat may never create, rename, move or remove it. What lies below it is held as before, row by row.

Claude Code's own sandbox holds only what lies below a path, so it is not handed the entry. That stays as it is until the operator rules on where purlis's trusted state lives.
