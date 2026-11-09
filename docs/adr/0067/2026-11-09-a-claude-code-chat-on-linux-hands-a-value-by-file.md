# Amended 2026-11-09: a Claude Code chat on Linux hands a value by file

Taken under the operator's standing instruction (the recommended path at every choice), 2026-11-09.

1. **A sandboxed Claude Code chat on Linux on Claude Code's own sandbox hands a value to a command
   by `--file` alone.** Its
   `purlis secret exec` with any `--env NAME=key` is refused by the app before any value is read or
   anything is run, a persona's declared tool and an approved MCP server's line among them. The
   refusal says what to run instead: `--file NAME=key`, with the command reading the value from the
   file `NAME` names, and a tool declared `as = "NAME file"`.
2. **Every other chat is as before**: a Claude Code chat purlis wraps whole (on a runner, or on a
   profile for sandboxed chats only), whose harness runs inside the wrap with its commands, as
   Codex and opencode chats on Linux do; every chat on macOS; and a chat started without the
   sandbox, which runs its `secret exec` itself. A chat's start records which it is
   (`Confines::whole`), and a record that does not say counts as Claude Code's own sandbox.
3. **What it costs.** A program that takes its credential from a variable alone cannot be handed it
   in such a chat. Run it in a chat purlis wraps whole, or in a terminal of your own. A GitHub App's
   token, handed as `GH_TOKEN`, is among them.
