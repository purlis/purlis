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

```bash
purlis worktree add <repo> <piece>     # cuts workspaces/<ws>/.worktrees/<repo>/<piece> on branch <piece>
cd workspaces/<ws>/.worktrees/<repo>/<piece>
purlis worktree update                 # rebase onto the base; --continue / --abort after a conflict
purlis worktree commit -m "<message>" <paths>   # or --all for every tracked change
purlis worktree push                   # pushes the branch after purlis's scan of what it publishes
purlis worktree done                   # finished; or: purlis worktree abandon "<why>"
```

- `add` takes `--base <branch>` to start from something other than the default branch. Exit 2
  means the piece name is already claimed, so pick another name.
- Run `commit`, `update`, `push`, `done` and `abandon` from inside the piece. In a sandboxed
  chat the app runs them outside the sandbox, and it goes by the folder the chat was started
  in, not the one it `cd`'d into. A chat started above the piece names it instead:
  `purlis worktree commit -m "<message>" .worktrees/<repo>/<piece>` commits every change in it,
  or name paths under it, relative to the chat's folder (an absolute path is refused). Run
  `commit` alone in its call, with no `cd` or other command beside it: purlis refuses it
  otherwise. Change folder in a call of its own first. `commit` only commits: no amend, reset, rebase or
  push, and the repo's own hooks don't run. A chat outside the sandbox can use plain git.
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
