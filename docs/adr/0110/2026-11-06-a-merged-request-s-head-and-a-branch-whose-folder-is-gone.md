# Amended (2026-11-06, #5439): a merged request's head, and a branch whose folder is gone

This amends §1 (what a finished folder is), §4 (a carried branch is deleted) and §5 (`purlis
worktree prune`), from the live test of #5209 on a workspace of about 290 folders.

## A tip inside a merged request's head is finished

§1 said every commit is on its base or carried there. A branch whose request got more commits
after the folder's tip, and then squash-merged, is neither: the tip's files are not what landed,
and the remote branch went with the merge. Yet nothing is lost: its commits are part of what
merged. The grill's first round had this rule ("an ancestor of a PR head that merged"), and it
was lost when §1 was written.

So a branch neither on its base nor carried is asked of the forge: **the newest request whose
head is the branch merged, and the tip is an ancestor of that request's head**
(`git merge-base --is-ancestor <tip> <head>`). Where the clone does not have the head, it is
fetched by the request's own ref, `refs/pull/<n>/head` on GitHub and
`refs/merge-requests/<n>/head` on GitLab, into the clone's objects alone. Offline, or with no
forge for the clone, nothing can be told: the folder is not finished, and the refusal says the
forge could not be asked. Such a branch is deleted with `git branch -D`, as a carried one is.

Only the newest request on the branch is asked, as everywhere purlis reads a branch's request
(`by_head`, ADR 0060). An older merged request under a newer one that is open or closed does
not count.

## A branch purlis cut whose folder is gone

A folder removed by hand, by another chat or by an older purlis leaves its branch. §5's prune
read folders only, and `remove --delete-branch` on such a folder refused, with a sentence
pointing at a folder that was gone.

- **`purlis worktree prune`** also reads every branch purlis cut (one with
  `branch.<name>.charterBase` recorded) that no folder of the clone is on. It deletes one that
  is on its base, carried, or inside a merged request's head, and keeps every other, saying why.
  A branch a task that has not ended names is kept. `--keep-branches` leaves them all, and
  `--dry-run` lists them.
- **`purlis worktree remove --delete-branch`** on a folder already gone clears what git
  recorded of it, and deletes its branch by the same reading, or keeps it, saying why.
- In a chat, the app runs both by the brokered route, each deletion in the checked clone, so
  git removes the branch's `branch.<name>` section of `.git/config` with it. A sandboxed chat's
  own `git branch -D` cannot write that file.

## A chat's prune shows life

A chat's prune waits on a silence, not on the whole prune. The app says it is alive (#4080)
after each folder it reads, each folder's links read and each removal, and reads folders
several at once. The 290-folder dry run that ran out the chat's one 120-second wait took
21 minutes in the app.
