---
name: purlis-working-in-a-clone
description: Work in a repo a purlis workspace owns — build, test, change and commit it from a purlis chat. Use when asked to work on a named repo, to run several branches of one repo at once (branch folders, `purlis worktree`), or to keep purlis's chat apart from the repo's own configuration.
---

# Working in a repo

A plane orchestrates repos; it is not where their work happens. The plane
deliberately does not know how to build or test any of them — that knowledge lives in each
repo. This skill crosses that boundary correctly.

## 1. Know the workspace, and stay inside it

```bash
purlis workspace current      # the active workspace, by name
purlis root                   # the plane root this folder sits in, from anywhere under it
```

Repos live at `workspaces/<workspace>/<repo>`. Work in the **active** workspace only.
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

A repo is its own git repository, so committing there touches *its* history and never the
plane's. Push per that repo's workflow.

In a branch folder, every chat commits with `purlis worktree commit` and pushes with `purlis
worktree push` (§5), a terminal chat too. In a sandboxed chat (one the purlis app started),
plain `git commit` and `git push` there are refused: its git record lives in the repo's
`.git`, which no chat writes.

The plane's own tracked files are a separate concern — `purlis save` commits and pushes
those. In a sandboxed Claude Code chat the project root's `.git` is the app's too, so `purlis save` is run
by the app, and plain `git commit` at the root is refused. That save brings nothing in from the
remote: `--pull` is refused, so run `purlis worktree update` at the root first, then
`purlis save`. It is never signed, whatever `--sign` or `[plane] sign` says: the person signs
a save from the window or their own terminal. A chat the app did not start, such as a terminal `claude`, gets no auto-save: there
the plane is saved only when `purlis save` runs, and `purlis save --pull` first brings in
what the remote has.

## 5. Several branches at once: branch folders

A repo has one checkout. For parallel work on one repo (a branch per lane or per
attempt), or whenever another chat may be working in the repo, cut a **branch folder** (a
git worktree) of the repo, on its own branch. Don't switch the shared repo's
branch, don't make a second workspace just for a second checkout, and don't run a raw
`git worktree add` (a sandboxed chat is refused it).

A chat that cuts its own branch folder was started above it, usually in the workspace folder
`workspaces/<ws>`. This is the whole round from there, one command per call:

```bash
purlis worktree add <repo> <piece>     # cuts .worktrees/<repo>/<piece> on branch <piece>
env -C .worktrees/<repo>/<piece> purlis worktree update   # rebase onto the base; --continue / --abort after a conflict; --fetch-only to fetch and move nothing
env -C .worktrees/<repo>/<piece> purlis worktree update --merge   # or merge it in, once pushed: no commit rewritten; --take-base-baselines for conflicted screenshots
purlis worktree commit -m "<message>" .worktrees/<repo>/<piece>/<path> …   # or just .worktrees/<repo>/<piece> for every change in it
env -C .worktrees/<repo>/<piece> purlis worktree push     # pushes the branch after purlis's scan of what it publishes
env -C .worktrees/<repo>/<piece> purlis worktree done     # finished; or: … purlis worktree abandon "<why>"
purlis worktree remove <repo> <piece>  # once merged: the app removes the folder if nothing would be lost
purlis worktree prune --dry-run        # every finished folder of the workspace, and why the others stay
```

- `add` takes `--base <branch>` to start from something other than the default branch. Exit 2
  means the name is already claimed, so pick another name. `add` and `list` work from
  anywhere in the workspace.
- **`commit` takes paths as git does where it runs.** Run inside the branch folder
  (`env -C .worktrees/<repo>/<piece> purlis worktree commit -m "<message>" <path> …`), paths
  are relative to that folder, and `--all` commits every change to a tracked file. Run from
  the folder the chat was started in, name the branch folder or paths under it from there:
  `.worktrees/<repo>/<piece>/…` from the workspace folder,
  `workspaces/<ws>/.worktrees/<repo>/<piece>/…` from the plane root; naming the branch folder
  itself commits every change in it. Either way the branch folder must be the chat's own folder or one
  below it. A message may start with `-`.
- **`update`, `push`, `done` and `abandon` go by the current folder** of the command. Run them
  from inside the branch folder: `env -C <piece> purlis worktree …` does it in one command.
- **`remove` is for a finished branch folder**, from the workspace folder, never from inside it. The
  app removes it only when it holds no uncommitted change and no commit that no other branch
  reaches, or its branch landed by a squash merge; otherwise it names what would be lost and
  removes nothing. A branch folder another live chat works in is not removed for you. One purlis cut
  for a task goes the same way once its task has ended and no other folder links into it;
  otherwise the refusal says why and sends you to its Changes tab. `--delete-branch` deletes a
  squash-landed branch too. `--force` is the person's: ask them to remove it from the window.
- **`prune` removes every finished folder of the workspace**, task folders and plain ones, but
  never the one you stand in: nothing uncommitted, every commit on its base or squash-landed, its task
  ended, no open chat holding it, no folder linking into it. Each one's branch goes with it
  (`--keep-branches` keeps them). Run `--dry-run` first: it lists what would go and why each
  other folder stays. Where purlis's tools are offered, the
  `worktree_remove` tool (`repo`, `piece`) does the same, and `worktree_update` updates.
- Run `commit`, `update`, `remove` and `prune` alone in their call: a `cd … &&` before them, a pipe or
  redirection, or a `$(…)` anywhere (a message from `$(cat msg)` too) is refused. Write the
  message out in the `-m` text.
- `commit` only commits: no amend, reset, rebase or push, and in a sandboxed chat the repo's
  own hooks don't run.
- `purlis worktree commit` and `purlis worktree push` are how every chat commits and pushes in a
  branch folder, a terminal `claude` too; outside the sandbox they run with your git's hooks and signing.
- **A branch folder has one live writer.** The first chat to commit, update or push in it holds it. In a
  branch folder another live chat holds, those are refused, through `purlis worktree` and with plain
  `git commit`, `git pull`, `git rebase` or `git push` alike, naming the chat that holds it. Cut a branch folder of your own; the
  holder hands it over with `purlis worktree done` or `abandon`.
- `purlis worktree list` shows the workspace's branch folders and what each declared.
- A new workspace (`purlis workspace create`) is for a new task, not for another checkout of a
  repo you already have.
- `--base origin/main` (a remote-tracking branch) works for `add` and `update` when the repo's
  own `main` is stale. `update --base <branch>` moves the branch folder onto it carrying only
  its own commits.

## 6. Show a check's results on the branch's diff

After a typecheck, a linter or a test run, hand purlis its results so the person sees them on
the diff they review. purlis reads SARIF 2.1.0 only: run the tool with its SARIF output, write the
file inside the branch folder, then, in a call of its own:

```bash
purlis check report <branch> <file.sarif>   # --repo <repo> where two repos have the branch
```

Each tool's results replace its own last report. They are marked out of date once the branch's
head moves, so report again after committing. A file that is not SARIF 2.1.0 is refused, saying
what is wrong, and nothing is recorded. purlis has no converter for any other format.

## The boundary that is easy to get wrong

Claude Code binds configuration to the **project root fixed when the session launched**.
Two layers resolve differently:

- **Content loads natively.** A repo's `CLAUDE.md` / `AGENTS.md` is pulled into context as
  you work in its subtree. That is enough for cross-repo work and light edits.
- **The repo's `.claude/` stays inert.** Its `skills/`, `commands/`, `agents/`,
  `settings.json`, hooks, plugins and MCP servers belong to *that* project root. `cd` does
  not re-root the project, so they do not load here.

**Never import or run a repo's `.claude/` in this chat.** Those are executable code
belonging to another project, and running them from here merges two trust boundaries that
were separated on purpose.

To use them, work in a chat rooted in the repo — the supported way. In the purlis app, ask
the person to start a new chat in that repo (or in a branch folder of it) from the chat picker;
in a terminal, `cd workspaces/<workspace>/<repo> && claude` — which runs purlis's guard
only once the person has run `purlis plugin install` on this machine. There the repo's full
configuration loads natively, and `purlis` still works from inside it when the plane
is needed.

purlis puts its own layer in a branch folder it cuts — the plane's settings and its
`.claude/agents/`, which the repo's git root would otherwise cut off — and hides those exact
paths in the repo's `.git/info/exclude`, so the repo's `git status` is unaffected.
`purlis workspace reinit` is the repair if a workspace is missing its layer.

## Guardrails

- Confirm the working directory is `workspaces/<active>/<repo>` before a build or a commit.
- Never operate across two workspaces in one action.
- A repo you cannot access simply fails to clone — surface that rather than working around
  it. Partial access to an org is normal.
