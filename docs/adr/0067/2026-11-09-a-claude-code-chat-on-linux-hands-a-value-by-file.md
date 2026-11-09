# Amended 2026-11-09: a Claude Code chat on Linux hands a value by file

Taken under the operator's standing instruction (the recommended path at every choice), 2026-11-09.

1. **A sandboxed Claude Code chat on Linux hands a value to a command by `--file` alone.** Its
   `purlis secret exec` with any `--env NAME=key` is refused by the app before any value is read or
   anything is run, a persona's declared tool and an approved MCP server's line among them. The
   refusal says what to run instead: `--file NAME=key`, with the command reading the value from the
   file `NAME` names, and a tool declared `as = "NAME file"`.
2. **Every other chat is as before**: Codex and opencode chats on Linux, every chat on macOS, and a
   chat started without the sandbox, which runs its `secret exec` itself.
3. **What it costs.** A program that takes its credential from a variable alone cannot be handed it
   in such a chat. Run it in a Codex or opencode chat, or in a terminal of your own.
