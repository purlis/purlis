# Amended (2026-11-28, #5476, #7406, #8008): one finished test for every cleanup, and a package's logs

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
writes, and `site/src/content/docs/docs/`, which the site's build copies from `docs/`. This
amendment lets the first go; the second still keeps the folder (decision 3).

## The decision

1. **Every cleanup reads `purlis_core::finished`**: tidy, the explorer's Remove, `remove` and
   `prune`. A folder one calls finished the others do too, and one kept is kept for the same
   reason everywhere: a claim log or a link that cannot be read keeps it in all four.
   - **Tidy** takes away an ended task's folder where it is finished (`finished::as_it_stands`: its
     files, its commits, the forge, the links into it) and it holds no git state of its own
     (#1472). Each guard it had stays: the task has ended, no open chat holds the folder by its
     claim or stands in it (read at a chat's close and a task's merge as at a project's open), the
     folder is still on the branch purlis cut, and its git runs in a checked clone and folder. A
     chat whose program has ended no longer holds a folder by its claim, but while it is open it
     still keeps the folder it stands in. What tidy cannot read keeps the folder (fail closed): a
     claim log of the workspace that cannot be read, an open chat with no id a claim could name, or
     a folder of the workspace whose links `git status` cannot list. A claim or a link nobody could
     read is not taken as none. A folder on its base goes with its branch, as before. One carried,
     found in history, or inside a request's head (one merged, or one closed with its head on
     origin, as §1 reads it since #4500) goes and **its branch stays**: tidy deletes a branch only
     where `git branch -d` does. `prune` deletes such a branch later, as one whose folder is gone
     (amended 2026-11-06).
   - **The explorer's Remove** removes a finished folder past git's guard: a task's folder, as
     before, and a plain one that no chat holds (read as a terminal reads it, with no word of
     which chats are open) and no other folder links into. The window still deletes no branch,
     `--force` is never passed on the person's behalf, and a folder that is not finished, or a
     plain one a chat holds, is removed as git removes it, or refused, as before.
2. **A package's logs are a build's**, as `target/` beside `Cargo.toml` is: a `logs/` folder
   beside a `package.json` that holds only `*.log` files.
3. **No ignored folder counts as a build's copy of a tracked one.** A rule that matched a
   folder's files to tracked files by path (#8002, #8008) was withdrawn before it merged: a
   path cannot tell a build's copy from the person's own files. A top-level `config/` that
   tracks only `default.json` would make a person's ignored `packages/api/config/default.json`
   count as a build's, and so would a copy made by hand (`cp -r docs site/docs`). So
   `site/src/content/docs/docs/` keeps a finished folder, as every ignored folder this
   amendment does not name does, until a repo can declare the folders its build writes again
   (#8013).

## Why

- **The definition was one; its readings were not.** ADR 0110 gave one module for every caller
  to ask, and tidy and the window's removal each kept a reading of their own beside it. The
  person saw the window refuse what the terminal removed, and a task's folder stay after its
  request merged.
- **Keeping too much is cheap; removing too much is not.** A folder kept for a build's copy costs
  the person one removal by hand. A person's file taken for a build's copy is gone. Only the
  repo knows which of its ignored folders its build writes, so it will declare them (#8013), and
  purlis does not guess from paths.
- **Tidy keeps branches for now; deleting them is deferred, not refused.** Wayfinder decision
  #7992 (operator, 2026-11-28) settles that after a merge the folder goes and its branch, on
  its base or carried there, is deleted, and that a branch is kept only while it is neither.
  This amendment does the first half: the folder goes. Deleting such a branch from a close, an
  open or the timer, where the person asked for nothing at that moment, belongs with the rest of
  that lifecycle, so it is deferred to the folder-lifecycle tickets of map #7990, which carry
  #7992 out. Until they are done, `prune` deletes such a branch where it loses nothing, when
  the person or a chat runs it.

## Considered and not taken

- **A list of each repo's generated paths in purlis's core.** It would name purlis's own folders
  there. A repo's own declaration (#8013) holds for any repo.
- **Match an ignored folder to a tracked one by path** (#8002, #8008). Withdrawn: it removed a
  package's own ignored `config/default.json` where the top tracks one of that name, and a hand
  copy of `docs/`, neither of which any build writes again.
- **Count a folder's commits by patch** (`git cherry`). A branch rebased before its request
  merged holds commits that are copies of the head's; on 2026-11-28 every clean folder kept so
  in smart-ide still had at least one commit no merged head held by patch. Not taken until a
  folder shows a need.
