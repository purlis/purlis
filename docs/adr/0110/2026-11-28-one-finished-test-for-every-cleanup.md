# Amended (2026-11-28, #5476, #7406): one finished test for every cleanup, a build's logs and copies

This amends §1 (which ignored files a build writes), §3 (the explorer's Remove) and the *One
reading for every caller* reason. It is lane 1 of the wayfinder map for one Repos view and the
branch-folder lifecycle.

## Where purlis was

Three cleanups read three different tests of whether a folder was done with:

- **The taking away of an ended task's folder** (`dispatchplace::tidy`, at a chat's close, a
  task's merge and a project's open) had its own test. It knew a branch on its base and a
  carried one, but never asked the forge, so a folder inside a merged request's head was kept
  for ever. It refused every ignored path but purlis's own layer, so a folder that had built
  (`target/` beside `Cargo.toml`, `node_modules/`, `dist/`) was kept for ever too.
- **The explorer's Remove** used the finished test only to let a task's folder past its own
  guard, then removed it as git does. git's guard does not see a carried branch, or one inside
  a merged request's head, as merged, so the window refused a folder `purlis worktree remove`
  took without `--force` (#5476).
- **`remove` and `prune`** read `purlis_core::finished`, as §5 and the 2026-11-06 amendment say.

And two folders a build writes kept a finished folder (#7406): `app/logs/`, which the e2e run
writes, and `site/src/content/docs/docs/`, which the site's build copies from `docs/`.

## The decision

1. **Every cleanup reads `purlis_core::finished`**: tidy, the explorer's Remove, `remove` and
   `prune`. A folder one calls finished the others do too, and one kept is kept for the same
   reason everywhere.
   - **Tidy** takes away an ended task's folder where it is finished (`finished::as_it_stands`:
     its files, its commits, the forge, the links into it) and it holds no git state of its own
     (#1472). Each guard it had stays: the task has ended, no chat holds the folder or stands in
     it, the folder is still on the branch purlis cut, and its git runs in a checked clone and
     folder. A folder on its base goes with its branch, as before. One carried, found in
     history, or inside a request's head (one merged, or one closed with its head on origin, as
     §1 reads it since #4500) goes and **its branch stays**: tidy deletes a branch
     only where `git branch -d` does. `prune` deletes such a branch later, as one whose folder is
     gone (amended 2026-11-06).
   - **The explorer's Remove** removes a finished folder past git's guard: a task's folder, as
     before, and a plain one that no chat holds (read as a terminal reads it, with no word of
     which chats are open) and no other folder links into. The window still deletes no branch,
     `--force` is never passed on the person's behalf, and a folder that is not finished, or a
     plain one a chat holds, is removed as git removes it, or refused, as before.
2. **A build's logs and a build's copy of a tracked folder are a build's**, as `target/` beside
   `Cargo.toml` is:
   - a `logs/` folder beside a `package.json` that holds only `*.log` files;
   - an ignored folder inside a package (under a folder below the top that holds a
     `package.json`), not a dot-folder, named as a tracked folder at the top of the repo, each
     of whose files is a tracked file at the same path under that folder, and which holds every
     tracked file there of each kind (extension) it holds: `site/src/content/docs/docs/spec.md`
     beside a tracked `docs/spec.md`, with every other tracked page, and without the pictures
     the site does not copy. A file of its own, a link, or only some of a kind keeps the
     folder; so does a dot-folder such as `packages/web/.vscode/`, which is a tool's or the
     person's even where the top's tracked `.vscode/` holds the same files. A file is matched
     by its path, not its contents, since a build may rewrite each page, so a copy in no
     package (`scratch/docs/`) keeps the folder: an edited copy there may be the task's.

## Why

- **The definition was one; its readings were not.** ADR 0110 gave one module for every caller
  to ask, and tidy and the window's removal each kept a reading of their own beside it. The
  person saw the window refuse what the terminal removed, and a task's folder stay after its
  request merged.
- **A copy loses nothing.** Each file of a copied folder is made from a tracked file of the same
  name, so the next build writes it again; a page of its own is the task's and keeps the folder.
- **Tidy keeps branches for now; deleting them is deferred, not refused.** Wayfinder decision
  #7992 (operator, 2026-11-28) settles that after a merge the folder goes and the landed branch
  is deleted, and that a branch is kept only while it has not landed. This amendment does the
  first half: the folder goes. Deleting the landed branch from a close, an open or the timer,
  where the person asked for nothing at that moment, belongs with the rest of that lifecycle, so
  it is deferred to the folder-lifecycle tickets of map #7990, which carry #7992 out. Until they
  land, `prune` deletes such a branch where it loses nothing, when the person or a chat runs it.

## Considered and not taken

- **A list of each repo's generated paths.** It would name purlis's own folders in purlis's
  core. The copy rule reads what the repo tracks, and holds for any repo whose build copies a
  folder.
- **Count a folder's commits by patch** (`git cherry`). A branch rebased before its request
  merged holds commits that are copies of the head's; on 2026-11-28 every clean folder kept so
  in smart-ide still had at least one commit no merged head held by patch. Not taken until a
  folder shows a need.
