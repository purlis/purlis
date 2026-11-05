# A workspace's repos live in `repos/`, and both layouts are read

**Accepted 2026-11-05**, from the operator's grill of the workspace-files feature. It adds a term,
**Workspace files**, to `CONTEXT.md`, and raises the workspace layout version
(`.purlis-structure`) from 5 to 6.

## Where purlis was

- **A workspace's repos were cloned at its root**, `workspaces/<ws>/<name>/`, beside
  `workspace.md`, `memory/`, `sessions/`, `todos/` and whatever the person or a chat put there.
  Repos and workspace files were mixed, and a clone could take any name but `skills` and
  `skills.local`, `memory` and `repos` included.
- **No one place said where a clone is.** About twenty call sites joined
  `workspaces/<ws>/<repo>` by hand (`gitbroker.rs`, `chatpiece.rs`, `files.rs`, `runnercode.rs`
  and more), and about six found clones by walking the workspace folder for `.git`.
- **Every layout bump so far only added files.** `purlis workspace reinit` is additive by design;
  nothing has ever moved content in a workspace.

## The decision

1. **A new clone goes to `workspaces/<ws>/repos/<name>/`**, in every workspace, from the release
   that ships this, including a workspace whose other clones are still at its root. `repos` joins
   the reserved names. Branch folders stay at `workspaces/<ws>/.worktrees/<repo>/<piece>`: moving
   them would break git's links back to their clone for nothing.
2. **One function in the core says where a repo is**, and every call site and every scanner goes
   through it. It answers `repos/<name>` when that is a clone and the old `<name>` otherwise, and a
   test refuses a new hand-built `workspaces/<ws>/<repo>` path. It lands first, with no change a
   person can see.
3. **Both layouts are read for ever.** A clone at the root keeps working with nothing done.
   - A workspace may hold clones in both places, and `doctor` lists the ones at the root.
   - When both `<name>/` and `repos/<name>/` exist, `repos/<name>/` is the repo and `doctor`
     warns about the other.
   - A clone at the root that is itself named `repos` (a `repos/` with `.git` directly in it) is
     read as an old clone. A new clone into that workspace is refused until it is renamed, and
     `doctor` says how.
4. **Moving is the operator's choice, never automatic.** `doctor` has a row, *N repos outside
   repos/*, whose fix **Move into repos/** moves one workspace's clones in one step that is all or
   nothing:
   - it refuses while any chat is open in that workspace;
   - it runs `git worktree repair` so each branch folder finds its clone again;
   - it leaves every harness's own stores alone. Its confirmation names the chats started in the
     clone itself (not in a branch folder), whose harness can no longer resume them; their
     session records still open.
5. **The layout version is 6, with no `requires` entry.** A clone exists only on the machine that
   made it, and the plane commits only its name in `workspace.json`, so a teammate on an older
   build is never affected. An older build on the same machine does not see clones in `repos/`
   and may clone again at the root: some clones go missing, nothing is corrupted. A `requires`
   entry would open a teammate's whole plane read-only over files only one machine has.

## Why not

- **Move every clone on upgrade.** A chat whose working directory is a clone, a harness's resume,
  an open editor and every branch folder's link all hold the old path. The person would find them
  broken with nothing done.
- **Read only the new layout, after a migration.** Every plane on every machine would have to be
  migrated before its repos open again.
- **A `path` field in `workspace.json`.** It would put a machine's own layout into a committed
  file shared with teammates whose machines differ.

## Consequences

- `docs/plane-format.md` changes first: `### workspaces/<ws>/`, the `workspace.json` `repos`
  table (`name` is the clone's folder under `repos/`, or at the root for an old clone) and the
  structure stamp's version list.
- The session-start briefing gets one line saying the workspace's notes, scripts and artifacts go
  in `workspaces/<ws>/`, never inside `repos/`.
- The LIVE block of `.gitignore` is unchanged: `repos/` is outside its allowlist, so it stays out
  of the plane's commits like any clone did.
