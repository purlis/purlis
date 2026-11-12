---
name: purlis-safe-remove
description: When the person wants a purlis workspace or persona gone, save what it learned, then run the guarded remove. Use when asked to remove, delete, retire or archive one. Not for pruning memory (purlis-tidy-memory), a branch folder (purlis-repo-work) or a new persona (purlis-persona).
---

# Safe remove

A workspace or a persona holds lessons nobody wrote anywhere else: its memory, its charter,
its todos, the work in its repos. Removing it deletes all of that. Safe remove is three steps,
in order, and the person sees each one before the next.

You run at the plane root, because the directory being removed is going away.

## 1. Audit

Read everything the subject holds, and sort each thing into **promote**, **leave** or **ask**.

A workspace:

```bash
purlis workspace recall -w <name>       # its memory, oldest first
purlis ws todo -w <name>                # its open todos
purlis status -w <name>                 # its repos: uncommitted, unpushed, open branch folders
purlis artifact list -w <name>          # its pinned artifacts, the pages its team sees
```

and its `workspace.md`, and every folder in `workspaces/<name>/artifacts/`, pinned or not.

A persona:

```bash
purlis persona show <name>              # its charter
purlis persona recall <name>            # its memory
purlis persona curation list <name>     # the curation actions it declares
```

Done when every memory, every open todo, every repo with unpushed work and every artifact has a
verdict.

## 2. Promote

A **durable learning** is one that is still true and still useful after the subject is gone: a
decision and its reason, a gotcha, a verified fact about a system. Write each where the
`purlis-remember` skill says a fact goes, or into the plane's docs when it describes the plane
itself. A todo that still matters moves to the
workspace that will do it (`purlis ws todo -w <other> "<text>"`). An artifact the team still
uses is copied into `workspaces/<other>/artifacts/<folder>/` of the workspace that will keep it
and pinned there with `purlis artifact pin -w <other> <folder>`. A repo's unpushed work is
the person's call: name it and ask them with `ask_operator`.

Show the person what you promoted, what you are leaving behind and why, and ask them about
each thing you sorted into **ask**, with `ask_operator`. Done when they have said go.

## 3. Remove

```bash
purlis workspace remove <name>
purlis persona remove <name>
```

Each one guards what it would destroy: `workspace remove` refuses while a repo or a branch folder holds
work nothing else has (exit 2), and `persona remove` refuses while another persona extends or
uses the one being removed. A refusal is the answer: relay it in its own words and stop.
Never pass `--force` without the person's word in this chat.

**In a chat purlis started under its sandbox, the last step is the person's press in the
purlis window.** Neither command deletes anything there, because the sandbox would stop a
delete partway.

- `purlis workspace remove <name>` hands the name to purlis, which asks the person in the Inbox,
  and on this chat's pane when it is on screen: **Remove…** or **Keep**. Remove… opens the window's
  Delete workspace, and the person's press there is what removes. The command says the person
  has been asked and exits without removing anything. That is not a failure. Tell the person the
  question is waiting in the Inbox, then end your turn. purlis tells you what they decided on your
  next turn. Do not run it again, do not pass `--force`, and do not delete any part of the workspace
  another way. If they kept it, it stays unless they ask again.
- `purlis persona remove <name>` is refused: say so, and ask the person to remove the
  persona from the window.

Removing a persona deletes its whole directory, memory and curation actions with it. The
deletion is a change to the plane like any other and travels with its next save.
