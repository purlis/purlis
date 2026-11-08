---
name: purlis-permissions
description: When a purlis chat is stopped by a permission — the harness keeps asking before a command, purlis's sandbox blocked a host or a folder, or a purlis guard refused a command — say what stopped it and send the change to where purlis reads it — a purlis guard ask or allow rule, the sandbox Notice's Allow, or the person's own sandbox settings in the purlis window — never to the harness's own settings file or its sandbox commands, which purlis's settings outrank. Use when asked to stop asking about a command, always ask before one, allow a host or a folder, explain why something was blocked or refused, or list purlis's permission rules. Not for a credential or vault a command needs — that is purlis-secrets. Not for a website login — that is purlis-browser. Not for committing or pushing from a branch folder — that is purlis-repo-work.
---

# Permissions in purlis

Three different things stop a command in a purlis chat, and each is changed in its own place.
Find which one it was before saying anything:

| What the person saw | What stopped it | Where it changes |
| --- | --- | --- |
| The harness asked before running a command | A harness permission rule | `purlis guard ask` / `purlis guard allow` |
| A Notice says the sandbox blocked a host, a folder, a socket or a service | purlis's sandbox | The Notice, or Settings › Project › Sandbox |
| The command was refused with purlis's words, naming the way that works | A purlis guard | Nowhere: take the way it names |

purlis hands every chat it starts its own settings, and they outrank the harness's. So a rule
you write into the harness's settings file by hand is never the one that decides: you do not
edit `.claude/settings.json`, `.claude/settings.local.json`, `opencode.json` or Codex's
`config.toml` yourself, and you never suggest the person does. You do not suggest Claude Code's
`/sandbox`, `/sandbox exclude` or `excludedCommands` either: purlis's own settings outrank them,
so they change nothing here.

## 1. The harness keeps asking: guard rules

`purlis guard` writes force-prompt (`ask`) and stop-prompting (`allow`) rules into the file each
harness reads, in that harness's own syntax, for every harness at once, and keeps no list of its
own.

```bash
purlis guard list                            # this plane's ask and allow rules, by file
purlis guard ask 'terraform apply *'         # always prompt before it
purlis guard allow 'cargo test *'            # stop prompting for it
purlis guard allow 'cargo test *' --local    # this machine only, not committed
purlis guard report                          # put back the prompt before a report is filed
```

- A bare command is wrapped as a `Bash(...)` rule; a pattern that already names a tool
  (`Read(...)`, `WebFetch(...)`) is kept.
- **An `allow` rule reaches the plane root only.** A workspace's and a branch folder's settings
  carry the plane's `ask` rules, never its `allow`, because a grant copied sideways is a
  permission nobody gave. Say so when the person asks to stop prompting inside a workspace.
- **An `ask` rule is in force at once** in every workspace.
- Without `--local` the rule is committed for everyone on the plane's repo; with it, it is this
  machine's own. Ask which they mean when it is not clear.
- Codex has no command-pattern rules, and opencode has no machine-only file; the command says
  so for each, and writes the rest.
- If it answers that a file is not valid and nothing was written, show the person which file:
  purlis never repairs these files, and you do not either.

**A permission is the person's to give.** Show them the exact `purlis guard` line and let them
run it in their own terminal, or run it yourself only when they tell you to. In a sandboxed chat
the write is refused: that refusal is the sandbox keeping a chat from granting itself
permissions, not a block to ask about.

## 2. The sandbox blocked it: Notices

When the sandbox blocks a command, the person sees a Notice in the Inbox, and on this chat's
pane when it is on screen. It names what was blocked and offers what purlis can grant:

- **Allow for this chat**, **Always allow** (every chat of this project on their machine, or,
  for a host only, everyone in the project) or **Keep blocked**, for a host or a folder.
- For a local socket, a system service or a port, what allowing it means, said on the Notice.
- For a command run with a vault, the way that works instead.
- Where a policy forbids it, which policy and who set it, so the person knows whom to ask.

Your part is short: say what was blocked, by its host, path, socket or service name, and wait.
If they allow it, this chat restarts on the same conversation and is told to retry. Do not
retry before that, and do not look for a way round the block.

Where purlis grants nothing, the person alone may choose Start without the sandbox for this chat;
never suggest it, and never present it as something you can do.

**The chat's sandbox level is the person's alone.** They set it in **Settings › Project ›
Sandbox › Your level**. You cannot lower it, so never suggest you can, never write it in a file
for them to confirm, and never ask them to lower it. If they say they lowered it and this chat
is still sandboxed, do not guess why: ask them to read the reason on this chat's sandbox mark or
on its Notice.

**"Allow this host" before anything was blocked** is the person's too: **Settings › Project ›
Sandbox › Your hosts** (this machine) or **Persona hosts** (one persona's chats). **Network**
on the same page lists what chats here reached and what was refused; `purlis doctor` counts the
refusals.

## 3. A purlis guard refused it

purlis's own guards refuse a command before it runs: one that would print a secret, move a
branch out from under a branch folder, write purlis's state by hand, and the like. The refusal
says why and names the way that works (`purlis worktree commit`, a declared tool, a
`purlis secret exec`). There is no rule to lift: take the way it names, and read the refusal to
the person if they ask why.

## Answering "why was this blocked"

1. Read the words the chat got back: a harness prompt, a Notice, or purlis's refusal (the
   table above).
2. Say which of the three it was, what it was about, and where it changes.
3. Leave the change to the person. You never edit a harness's settings file to get past it.
