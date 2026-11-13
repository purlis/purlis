---
name: purlis-workspace
description: When the person wants a purlis workspace made, chosen, shared or saved (`purlis save`). Use when asked to create, fork, rename or share a workspace. Not for removing one (purlis-safe-remove), its memory (purlis-tidy-memory), a new chat (purlis-handoff) or a branch (purlis-repo-work).
---

# A workspace's life

A workspace is a folder under `workspaces/` with its charter (`workspace.md`), its manifest
(`workspace.json`: its repos and their branches), its memory, its todos and its clones. This
skill is every command that makes one, copies one, rebuilds one, renames one, picks one or
shares one, and the rule each command keeps.

Two of them **publish**: `purlis workspace live` and `purlis save` put the workspace's memory
and the plane's changes on its remote, where everyone with the plane reads them. Neither runs
without the person's yes in this chat, for this workspace.

## Which command

| The person wants | Command | Its rule |
| --- | --- | --- |
| a new workspace | `purlis workspace create <name> --vision "<the goal>"` | always with a vision; a sandboxed chat is refused `--live` and `--use` |
| a copy of this one | `purlis workspace fork <src> <new>` | copies charter, memory, todos and manifest; **never the repos** |
| one rebuilt from its manifest | `purlis workspace restore <name>` | clones each repo the manifest names and checks out its branch |
| a new name | `purlis workspace rename <old> <new>` | refused while a chat runs in it |
| to work in one | `purlis workspace use <name>` | locks the session to it |
| the one a session lands on | `purlis workspace default <name>` | `--clear` removes it |
| its repos and branches recorded | `purlis workspace snapshot <name>` | what `restore` and `fork --restore` rebuild |
| it shared | `purlis workspace live <name>` | **publishes**; `--off` makes it private again |
| the plane's changes pushed | `purlis save "<message>"` | **publishes** everything pending, not only yours |

`purlis workspace list` names every workspace; `purlis workspace current` names this chat's.

## Create

```bash
purlis workspace create <name> --vision "<what it is for, in one line>"
purlis workspace create <name> <repo> <repo> --vision "<...>"   # clones those repos into it
```

Ask what it is for when the person has not said. A workspace without a vision is never
proposed by a handoff, so it is created unfindable.

- **A sandboxed chat is refused `--live` and `--use`**, and nothing is created. Create it
  without them. To share it, ask the person to run `purlis workspace live <name>` in their own
  terminal. To work in it, name it with `-w <name>` on each command, or hand the work to a chat
  there with `purlis handoff <name> "<brief>"` (the `purlis-handoff` skill).
- A chat started at the plane root stays there: `--use` is refused, so name the new workspace
  with `-w <name>` from here.
- Outside the sandbox, `--use` selects it for this session and `--live` shares it from birth,
  which publishes, so it waits for the person's yes.

## Fork

```bash
purlis workspace fork <src> <new>             # charter, memory, todos and manifest
purlis workspace fork <src> <new> --restore   # and clone its repos now
```

**A fork does not copy the repos.** The clones, their uncommitted work and their branch folders
stay in the source; the fork has the manifest, so `--restore` (or a later `purlis workspace
restore <new>`) clones each repo again and checks out the branch it records. Work not yet pushed
is not in the fork: say so before forking a workspace with work in flight. Run `purlis workspace
snapshot <src>` first if its branches moved since the manifest was written.

A fork is LOCAL unless `--live` is given, and `--live` publishes. A sandboxed chat is refused a
fork, because it rewrites a manifest: ask the person to run it in a terminal.

## Restore and snapshot

`purlis workspace snapshot [<name>]` records the workspace's repos and branches into its
manifest. It refuses branches that would not restore; `--force` records them as they stand, and
only with the person's word. `purlis workspace restore <name>` rebuilds from that manifest:
a clone per missing repo, then its branch checked out and pulled. `--on-demand` clones each repo
only when it is entered.

## Rename

```bash
purlis workspace rename <old> <new>
```

It moves the directory, its clones' branch folders and every record that names it. **It is
refused while a chat is running in the workspace**, under either name, and that includes this
chat when it works there. It is refused in a sandboxed chat too, because it rewrites the
manifest. So ask the person to run it in their own terminal, once the chats in it are closed. An interrupted rename is finished by running the same command again.

## Use and default

`purlis workspace use <name>` selects a workspace for this terminal and session and **locks** the
session to it; `purlis workspace unlock` releases the lock. A sandboxed chat stays in the place it
was started, and a chat started at the plane root is never moved: name the workspace with
`-w <name>` instead, or start a chat in it.

`purlis workspace default <name>` sets the workspace a session lands on when nothing else
decided; with no name it prints it, and `--clear` removes it.

## Live and `purlis save` publish

A workspace is LOCAL when it is created: its charter, manifest and memory stay on this machine.
`purlis workspace live <name>` makes it **LIVE**: those files are committed, shared with
everyone who has the plane, and saved from then on. `purlis workspace live <name> --off` makes it
private again, and stages the removal of what was shared; `purlis save` then publishes that.

`purlis save "<message>"` commits and pushes **everything pending in the plane**, not only what
this chat changed, and prints what it is about to commit. In a chat the app started, the app
saves it; in a terminal, the chat runs `purlis save` only on the person's yes. `--pull` first
brings in what the remote has; `--no-push` commits only.

Before either one:

1. Say what will be published: the workspace's memory, todos and charter for `live`; the
   pending files for `save`.
2. Ask the person, with `ask_operator`, and run it only on their yes in this chat.
3. A memory that holds something private stays out: move or forget it first.

## Not this skill

- Removing, deleting or retiring a workspace, which loses what it learned unless audited first:
  the `purlis-safe-remove` skill.
- Pruning, deduping or tidying a workspace's memory: the `purlis-tidy-memory` skill.
- Moving work into a new chat, in this workspace or another: the `purlis-handoff` skill.
- Forking a repo, cutting a branch or a branch folder, committing in a repo: the
  `purlis-repo-work` skill.
