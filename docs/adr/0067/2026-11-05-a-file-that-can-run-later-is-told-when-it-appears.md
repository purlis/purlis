# Amended 2026-11-05 (§5, class 5): a file that can run later is told when it appears

**Decided by the person, 2026-11-05.** What class 5 holds, stated exactly, and a second look for
what it does not.

## What holds

1. **A sandboxed chat can't write these names directly** in the folders it may write (`PLANTED`,
   and the project's manifests where they can change a chat's sandbox, as amended 2026-10-06).
   That is a refusal, and it stays one. `tools/seatbelt-case.sh` holds it through the real
   harness at each level on macOS.
2. **A name that arrives another way than a direct write is detected, not refused**, by the look
   below, within its bounds. Earlier text of this ADR and of `SECURITY.md` that reads as a
   guarantee of more than item 1 is narrowed to this.
3. **A temp folder or a cache home is not held by these names, and stays that way.** Earlier text
   that reads as if class 5 held them is narrowed too. Tests and tools build repositories and
   manifests in temp folders all the time, and git can't start one under such a denial.
   `tools/seatbelt-case.sh` asserts that a repository and a manifest can still be made there, so a
   later change doesn't take that away. Neither is a folder this look walks.

## The second look

`purlis_core::sandbox::arrived`, for every harness.

- **When.** As a sandboxed chat starts, purlis records which names are in the folders it may
  write. At each end of its turn (`Stop`, on a thread of its own) it looks again. So what arrives
  is told at the next turn's end, and not while a turn runs. A chat that never ends a turn again
  is not looked at again.
- **What it tells.** One Inbox Notice a chat, naming the chat and each path once, saying that these
  can run outside the sandbox if the person or a tool works in that folder. Each look that finds
  something also writes `trust.sandbox.arrived` to the event log, with paths and no contents. Put
  away takes the Notice back. **Nothing is deleted or changed.**
- **What it says it does not know.** It says what appeared, never who put it there: a person's
  editor making `.idea` is told as anything else is.
- **The record** is `<state>/app/arrived/<chat>.json`, denied a chat's reading and writing for every
  harness under class 2, so a chat neither writes what decides what is told nor holds its lock. It
  survives a restart. A look that fails is said in the log, not to the person.

## Bounds

The look goes through at most 500,000 directory entries, level by level, and keeps at most 20,000
names. Measured: a branch folder of this repository is about 22,000 entries, while a workspace with
247 of them is over four million, so a chat at such a workspace's root meets the bound, and its look
covers the top of every folder and not the bottom.

- **At the bound it says so.** The Notice says the check is incomplete, once a set of folders until
  a look there is whole. A look cut short never reads as "nothing appeared".
- **A folder it can't read is named**, or a watched folder that is a link or isn't there. So is
  the next one, whatever was put away.
- **A name first reached below where a look has read whole isn't told**, because it can't be said
  to be new. It is known from then on.
- **The first look may overlap a chat's first moments.** A start waits a quarter of a second for it
  and then goes on, so on a very large folder what appears while it runs may count as there at the
  start.
- **A name already there stays known by its folder.** A file rewritten where it stands isn't told.
  A folder or `.git` that is now another one, or a name whose folder is now another one, is.
- **It does not look** in a temp folder or a package cache, in `node_modules` or `target`, or at
  anything below a name that was already there, so a change under a name already there isn't
  told.
- **At the floor** it walks the chat's folders and the project, and asks for the home's own names
  directly. It does not walk the rest of the home or the disk, though the floor writes them.
- **An empty file or folder is not told.** It runs nothing, and a harness's sandbox may put one
  there to hold a path. It is told once it holds something.
- A nested manifest is not told, as class 5 does not hold one (#1336).

## Nothing found is run

It follows no link, opens only regular files, starts no program and never runs git in anything it
found, as a repository's own config or hooks would run with it. What it reads of a repository it
reads by gitoxide, opened isolated.

## The app's own work

It is told apart by what is on disk and a chat cannot write, so it holds however a checkout is made
and nothing has to remember to say it acted:

- a new branch folder: its `.git` names a directory in a known repository's `.git/worktrees` that
  names it back;
- a new submodule: its `.git` names a directory in a known repository's `.git/modules`;
- a new clone: a workspace's manifest names the repo;
- a file in a branch folder: its index, in the clone's `.git/worktrees`, has that file;
- a file in a clone: one of its remote-tracking branches has that file. The clone's own index and
  commits are not enough.

A repository is known when its `.git` was there as the chat started, or by the above, and it is
still that `.git`. A `.git` file's directory must be in a `.git`'s own `worktrees` or `modules`.
Nothing else is opened. Each file must match what the repository has, by content, so the name alone
is never enough. These checks take 30 seconds at most, and past that every name is told.
Something these can't vouch for is told, which is a false alarm at worst: a person's own local
commit, for one.
