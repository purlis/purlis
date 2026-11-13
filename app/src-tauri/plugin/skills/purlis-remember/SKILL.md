---
name: purlis-remember
description: When something learned in a purlis chat should outlive it, record it or recall a decision. Use when asked to remember this, note that, add a todo, or what did we decide. Not for tidying memory (purlis-tidy-memory), a closing summary (purlis-smart-close) or a password (purlis-secrets).
---

# Remember

purlis keeps four memories a chat writes to, and later chats read them: the briefing shows the
newest titles, and a search finds the rest. A fact in the wrong one is not found by the chat
that needs it, or is told to chats it means nothing to. This skill holds the one rule for where a
fact goes; other purlis skills point here rather than saying it again.

## Where a fact goes

Ask who needs the fact next, then take the first that fits:

1. **A decision every later chat in this workspace should act on from its first turn**, and
   why → the workspace's charter, its `workspace.md`. Add it with the `workspace_section` tool
   and section `decisions` (`## Context & decisions`). A term this work coined takes section
   `glossary` (`## Glossary`), and a changed goal the `workspace_vision` tool, which replaces
   `## Vision`. A sandboxed chat is refused `workspace.md` itself; these tools write it for the
   chat.
2. **About this workspace's work**: a gotcha, a verified fact about a system it touches, what
   was tried and why it failed → this workspace's memory, with the `memory_add` tool.
3. **How this persona's role is done, true in any workspace** → this persona's memory, with the
   `persona_remember` tool.
4. **True for every persona, anywhere in this project** → shared memory, with the
   `persona_remember` tool and `shared`.

Then:

- **Never a secret.** Every later chat reads memory, and persona and shared memory are published
  with the project. A credential goes to a vault (the `purlis-secrets` skill); a memory may name
  the vault, never the value.
- **Not what the repo already records**: its code, its history, its docs. Record what the work
  *taught*, which nothing else says.
- **Work still to do is a todo**, not a memory (below).
- **At the project root**, or for a workspace about to be removed, there is no workspace to
  write to: a lesson goes to persona or shared memory.
- **One fact per entry.** Its first line is its title, and says the fact, not the topic.
- **Unsure between two**, take the narrower one. A memory in the wrong scope is moved later
  (the `purlis-tidy-memory` skill), keeping its title and date.

A harness without purlis's tools runs the commands. Name this chat's own workspace or persona,
the spelling that runs without a prompt, and backslash-escape each backtick in a double-quoted
argument, or the shell runs it:

```bash
purlis workspace remember -w <workspace> "<fact>"      # this workspace's memory
purlis workspace vision "<the goal>"                    # replaces the workspace's ## Vision
purlis persona remember <persona> "<fact>"             # this persona's memory
purlis persona remember <persona> --shared "<fact>"    # shared memory, for every persona
```

Done when the answer names the memory it wrote and its title.

## Recall

Before answering "what did we decide" or acting on a remembered fact, search; the briefing's
titles are only the newest.

```bash
purlis recall "<keywords>"                        # every memory at once, each hit labelled by source
purlis recall "<keywords>" --all-workspaces       # every workspace's memory, not only this one's
purlis recall "<keywords>" --since 2w --full      # recent ones, with a line of each body
purlis persona recall <persona> --query <keywords>
```

The `memory_search` tool searches this workspace's memory without a shell. A decision in the
charter is in `workspace.md`, which the briefing already gave this chat; a past chat's summary is
a session record (`session_record_list`, `session_record_read`).

Answer with where the fact came from and its date. A memory that names a file, a flag or a
command is checked against the code before anything is done on it: memory says what was true
when it was written. Done when the answer cites its sources, or says nothing was found.

## Todos

A todo belongs to the workspace this chat works in.

- `todo_list` shows the open ones, each with its slug.
- `todo_add` records one; it is refused when an open todo is already about the same work.
- `todo_done` closes one by its slug and writes its closing to the workspace's memory.

In a terminal:

```bash
purlis ws todo "<text>"                    # record one; -w <workspace> for another workspace
purlis ws todo                             # the open ones (--done for the done ones)
purlis ws todo done <slug>                 # undo: purlis ws todo reopen <slug>
```

## Promote a todo to an issue: ask first

`purlis ws todo promote <slug> --repo <repo>` opens an issue on the repo's forge, as the person,
with the todo's title and text, links the todo to it and closes the todo. An issue can be public
and cannot be taken back, so ask the person first with the `ask_operator` tool: name the todo,
the repo and who can read it. `--repo` takes the repo's name in this workspace, and may be left
out when the workspace has one repo on a forge. Run it only on their yes. Done when the answer
names the issue it opened.
