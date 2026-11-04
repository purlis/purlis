# A finished folder goes without `--force`, and a carried branch goes with it

**Accepted 2026-11-04**, from the operator's grill of #4245 and #5168. It amends the rule #1534
set (*"never a branch folder purlis cut for a task, `--force` or not"*), as `purlis worktree
remove`, the explorer's Remove and ADR 0067's amendment of 2026-10-30 (*a chat's finished branch
folder is removed by the app*) applied it. It adds a term, **finished folder**, to `CONTEXT.md`.

## Where purlis was

- **A task's folder could only be discarded from its Changes tab**, whatever its state. A task
  that ended with its branch on its base still left a folder that `purlis worktree remove`
  refused, from a terminal and from a chat, and the person cleared each one by hand (#5168).
- **A squash-landed branch stayed behind.** Discard, `--delete-branch` and the taking away of a
  merged task's folder delete a branch only where `git branch -d` finds it merged, and git never
  finds a squashed branch merged. `standing::landed` already reads such a branch as `Carried`
  (#4295), and nothing acted on that.
- **An ignored link kept a folder** (#4245). Lanes share an install by linking `app/node_modules`
  into another branch folder. git lists a link with no closing slash, so the rule that lets a
  build's folders go never matched it.

## The decision

1. **A finished folder** is a branch folder that meets all of these:
   - no uncommitted changes, less purlis's own layer;
   - every commit is on its base, or carried there by a squash merge (`Landed::Yes`,
     `Landed::Carried`, or one commit on its base making its whole change); a folder on no
     branch is at a commit its base has;
   - no ignored files beyond the folders only a build or the harness writes, and links;
   - its task, if any, has ended: done, failed, blocked, cancelled, stopped, or ended without a
     report. Working, needs you, asking and idle are not ended;
   - no open chat that has declared neither `done` nor `abandoned` holds it;
   - no other remaining folder links into it.

   *Landed* stays a change member's word. For a branch, say *on its base* or *carried*.
2. **Ignored links are disposable**, wherever they point. Removing a folder deletes the link and
   never its target.
3. **`purlis worktree remove` without `--force` removes a finished task folder**, from a chat or
   a terminal, and so does the explorer's Remove; `--delete-branch` goes with it. A task folder
   that is not finished is still sent to its Changes tab, and the refusal says why: its task has
   not ended, it holds uncommitted changes, commits its base does not have, ignored files, or
   another folder links into it. `--force` on a task's folder stays refused, and stays refused
   in any chat.
4. **A carried branch is deleted with `git branch -D`** wherever purlis deletes a branch:
   `--delete-branch`, Discard, and `purlis worktree prune`. Only where `standing::landed` reads
   it `Carried`, as the folder goes, and only the local branch: a remote branch is the forge's.
5. **`purlis worktree prune -w <ws> [repo] [--dry-run] [--keep-branches]`** removes every
   finished folder of a workspace, task folders and plain ones, in every repo or one. A folder
   that links into another goes before it. Each removed folder's local branch is deleted, unless
   `--keep-branches`. `--dry-run` lists what would go and every folder kept, with its reason. In a
   chat the app runs it, by the route a chat's `remove` takes (ADR 0067, amended 2026-10-30):
   from the workspace's folder or the project root, never from inside a branch folder, each
   folder's git run in its checked clone and folder, each removal held to its claim. A Claude
   Code chat carries `Bash(purlis worktree prune)` and `Bash(purlis worktree prune *)` among its
   own-scope rules, alone in its call as a removal is. Without the app, which chats are open
   cannot be told, so a folder a chat holds is kept, and said.

## Why

- **Nothing is lost by it.** Every rule of a finished folder is about what its removal would
  take: work (changes, commits, ignored files), someone at work in it (a task, a chat), or a
  folder that needs it (a link). Where all of them read clean, the Changes tab's Discard would
  show the person nothing to decide, and sending them there was a chore with no choice in it.
- **A carried branch holds nothing its base does not.** Keeping it because git's `-d` cannot see
  a squash left every squash-landed lane behind as a branch. purlis already reads it `Carried`
  before it lets the folder go without `--force`; deleting the branch on the same reading adds
  no new judgement.
- **One reading for every caller.** `remove`, the explorer and `prune` ask the same module
  (`purlis_core::finished`), so a folder one calls finished the others do too.

## Considered and not taken

- **Let `--force` remove a finished task folder.** A finished folder needs no force, and keeping
  `--force` refused on a task's folder keeps the one act that discards work in the window.
- **Read a closed chat's folders from the terminal as free.** The terminal cannot ask the app
  which chats are open, so `prune` there keeps a folder a chat holds and says so; the holder's
  `done`, or a `prune` from a chat or the window, lets it go.
