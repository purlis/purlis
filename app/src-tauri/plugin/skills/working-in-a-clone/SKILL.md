---
name: working-in-a-clone
description: Do real work inside a repo a workspace owns — build, test, change, commit. Use when asked to work on a named repo from a control plane, to run several branches of one repo at once (branch folders, `purlis worktree`), and to get the boundary right between the plane's session and the repo's own configuration.
---

# Working inside a clone

A control plane orchestrates repos; it is not where their work happens. The plane
deliberately does not know how to build or test any of them — that knowledge lives in each
repo. This skill crosses that boundary correctly.

## 1. Know the workspace, and stay inside it

```bash
purlis workspace current      # the active workspace, by name
purlis root                   # the plane root this folder sits in, from anywhere under it
```

Clones live at `workspaces/<workspace>/<repo>`. Work in the **active** workspace only.
A repo needed by this task that sits in another workspace gets cloned into this one —
never reached across.

## 2. Make sure it is cloned

```bash
purlis clone <repo>           # skipped if already present; checks out its real default branch
```

`<repo>` is a name from the plane's inventory (`purlis discover` refreshes it). Default
branches differ across an org (`main`, `master`, `develop`); `purlis clone` reads each
repo's actual default branch, so never assume one.

## 3. Adopt the repo's own conventions before editing

Read whichever of `CLAUDE.md`, `AGENTS.md`, `README.md` the repo ships, then skim its
manifest (`package.json`, `pom.xml`, `pyproject.toml`, `go.mod`, `Cargo.toml`) for its real
build and test commands. Use those — never commands carried over from a different repo.

## 4. Commit to the repo you are in

A clone is its own git repository, so committing there touches *its* history and never the
plane's. Push per that repo's workflow.

In a sandboxed chat (one the purlis app started), plain `git commit` and `git push` in a
branch folder are refused: its git record lives in the clone's `.git`, which no chat writes.
Use `purlis worktree commit` and `purlis worktree push` there (§5).

The plane's own tracked files are a separate concern — `purlis save` commits and pushes
those. A chat the app did not start, such as a terminal `claude`, gets no auto-save: there
the plane is saved only when `purlis save` runs, and `purlis save --pull` first brings in
what the remote has.

## 5. Several branches at once: branch folders

A clone has one checkout. For parallel work on one repo (a branch per lane or per
attempt), or whenever another chat may be working in the clone, cut a **piece**: a branch
folder (a git worktree) of the clone, on its own branch. Don't switch the shared clone's
branch, don't make a second workspace just for a second checkout, and don't run a raw
`git worktree add` (a sandboxed chat is refused it).

A chat that cuts its own piece was started above it, usually in the workspace folder
`workspaces/<ws>`. This is the whole round from there, one command per call:

```bash
purlis worktree add <repo> <piece>     # cuts .worktrees/<repo>/<piece> on branch <piece>
env -C .worktrees/<repo>/<piece> purlis worktree update   # rebase onto the base; --continue / --abort after a conflict
purlis worktree commit -m "<message>" .worktrees/<repo>/<piece>/<path> …   # or just .worktrees/<repo>/<piece> for every change in it
env -C .worktrees/<repo>/<piece> purlis worktree push     # pushes the branch after purlis's scan of what it publishes
env -C .worktrees/<repo>/<piece> purlis worktree done     # finished; or: … purlis worktree abandon "<why>"
```

- `add` takes `--base <branch>` to start from something other than the default branch. Exit 2
  means the piece name is already claimed, so pick another name. `add` and `list` work from
  anywhere in the workspace.
- **`commit` takes paths as git does where it runs.** Run inside the piece
  (`env -C .worktrees/<repo>/<piece> purlis worktree commit -m "<message>" <path> …`), paths
  are relative to that folder, and `--all` commits every change to a tracked file. Run from
  the folder the chat was started in, name the piece or paths under it from there:
  `.worktrees/<repo>/<piece>/…` from the workspace folder,
  `workspaces/<ws>/.worktrees/<repo>/<piece>/…` from the plane root; naming the piece's own
  folder commits every change in it. Either way the piece must be the chat's own folder or one
  below it. A message may start with `-`.
- **`update`, `push`, `done` and `abandon` go by the current folder** of the command. Run them
  from inside the piece: `env -C <piece> purlis worktree …` does it in one command.
- Run `commit` and `update` alone in their call: a `cd … &&` before them, a pipe or
  redirection, or a `$(…)` anywhere (a message from `$(cat msg)` too) is refused. Write the
  message out in the `-m` text.
- `commit` only commits: no amend, reset, rebase or push, and the repo's own hooks don't run.
- `purlis worktree commit` is for a sandboxed chat. A chat outside the sandbox, such as a
  terminal `claude`, is told to use git: it commits with plain `git commit` in the piece, and
  pushes with `git push` or `purlis worktree push`.
- `purlis worktree list` shows the workspace's pieces and what each declared.
- A new workspace (`purlis workspace create`) is for a new task, not for another checkout of a
  repo you already have.

## The boundary that is easy to get wrong

Claude Code binds configuration to the **project root fixed when the session launched**.
Two layers resolve differently:

- **Content loads natively.** A clone's `CLAUDE.md` / `AGENTS.md` is pulled into context as
  you work in its subtree. That is enough for cross-repo work and light edits.
- **The clone's `.claude/` stays inert.** Its `skills/`, `commands/`, `agents/`,
  `settings.json`, hooks, plugins and MCP servers belong to *that* project root. `cd` does
  not re-root the project, so they do not load here.

**Never import or run a clone's `.claude/` in this session.** Those are executable code
belonging to another project, and running them from here merges two trust boundaries that
were separated on purpose.

To use them, work in a chat rooted in the repo — the supported way. In the purlis app, ask
the operator to start a new chat in that clone (or in a worktree of it) from the chat picker;
in a terminal, `cd workspaces/<workspace>/<repo> && claude` — which runs purlis's guard
only once the operator has run `purlis plugin install` on this machine. There the repo's full
configuration loads natively, and `purlis` still works from inside it when the control plane
is needed.

purlis puts its own layer in a worktree it cuts — the plane's settings and its
`.claude/agents/`, which the clone's git root would otherwise cut off — and hides those exact
paths in the clone's `.git/info/exclude`, so the repo's `git status` is unaffected.
`purlis workspace reinit` is the repair if a workspace is missing its layer.

## Guardrails

- Confirm the working directory is `workspaces/<active>/<repo>` before a build or a commit.
- Never operate across two workspaces in one action.
- A clone you cannot access simply fails to clone — surface that rather than working around
  it. Partial access to an org is normal.
