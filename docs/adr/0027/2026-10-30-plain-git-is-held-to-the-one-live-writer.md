# Amended 2026-10-30: plain git is held to the one live writer (#3931)

The amendment of the same day (#3901) left a gap: **plain git was outside the claim.** A chat
started without the sandbox commits, rebases and pushes with git itself, which purlis does not
broker, so two writers could still meet in one branch folder.

**Now the Bash guard of a chat the app started without the sandbox holds plain git to the same
claim.** Before a command runs, the guard reads it for git that writes a branch folder's branch
(`purlis_core::branchgit::branch_write`):

- a commit: `git commit`, `merge`, `cherry-pick`, `revert`, `am`;
- an update: `git pull`, `git rebase`;
- a push: `git push`.

A dry run, and an `--abort`, `--quit` or `--show-current-patch` of one under way, writes nothing
and is let through. The line is read and placed as the sandboxed chat's branch-folder arm reads
it (#3830): a `cd` to a literal folder is followed on a line run in one shell, `git -C` is
followed, and a move the guard cannot place leaves the line alone.

Where the line writes a branch folder purlis made, the guard asks the app the brokered write's
own question over the chat's hook socket (`GitWork::WorktreeClaim`). The app runs no git. It
judges the claim against the chats it has (`piececlaim::take`), as it judges a brokered write:

- a folder no live chat holds is taken by this chat;
- the holder's own git runs;
- a folder another live chat holds is refused, in the brokered refusal's words, naming the holder
  and how to hand it over. The refusal is tallied as `branch-folder-held`.

**Where no app answers, the piece log alone decides**, as `purlis worktree push` does with no
app: a folder another chat holds is refused, since whether that chat still works cannot be told
(fail closed). An app older than the ask drops the line it cannot read, which is the same case.
The guard waits for the app at most 1.2 s, inside its 2 s budget.

**The person is never held to it.** Their own terminal runs no guard. A harness they start
themselves has no chat number from the app and is not asked. A shell tab in the window is the
person's own and is never held to a claim, nor takes one.

A sandboxed chat is unchanged. Its plain `git commit` in a branch folder is already refused,
pointing at `purlis worktree commit` (#3830), which is held to the claim.

What this still does not cover, and is written down rather than closed:

- **It prevents accidents and is not a wall**, as the first amendment says: the piece log is
  in a workspace chats may write, and a line the guard cannot place is let through.
- **A chat's own scripts.** git run from inside a script, a build tool or an alias the line
  does not spell is out of the guard's sight.
