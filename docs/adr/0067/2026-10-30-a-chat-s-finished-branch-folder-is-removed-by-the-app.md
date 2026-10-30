# Amended (2026-10-30, #3879; extends "a branch folder is kept current by the app"): a chat's finished branch folder is removed by the app when nothing would be lost, and the removal asks no approval — ruled by the operator, 2026-10-29

**Ruled by the operator, 2026-10-29**: own-scope work runs without a prompt; when nothing
would be lost, purlis does the work for the chat, as it does commits and updates. A sandboxed
chat cleaning up its branch folders after a merge was refused (`git worktree remove` deletes
the folder's files before git's record of it, which the sandbox stops partway) and sent to the
person, while the standing rule tells it to clean up after every merge.

1. **`purlis worktree remove <repo> <piece> [--delete-branch]`, in a chat the app started**, is
   handed to the app as a brokered git action of its own kind (`GitWork::WorktreeRemove`),
   sandboxed or not, as `update` and `push` are. The app runs the terminal's own removal
   (`piececmd::remove`) in the checked clone and folder, every git call pinned as for an update.
2. **Never forced.** The ask has no word for `--force`. The removal is refused, naming what
   would be lost, while the folder holds an uncommitted change or a commit no other ref
   reaches, and a folder purlis cut for a task is refused as it is from a terminal.
   `--delete-branch` deletes the branch as `git branch -d` does, only where it is merged.
   `--force` from any chat is refused by the command before anything is asked, naming the
   window's *Remove folder*, since the person decides what is discarded.
3. **Only a branch folder of the chat's own workspace, from above it.** The workspace must be
   one the chat writes (it stands in it, or at the project root), and the chat must not stand
   in another branch folder, nor in the folder itself, whose removal would take the chat's own
   folder from under it. The ask names a repo and a piece, so it reaches only
   `<workspace>/.worktrees/<repo>/<piece>`: never the clone itself, nor the project root.
4. **No approval.** A Claude Code chat carries `Bash(purlis worktree remove *)` among its
   own-scope rules (ADR 0064 as amended for #3829), alone in its call as a commit is (a rider
   is refused), and the `worktree_remove` tool joins the own-scope tools. The rule matches
   `--force` too, which is safe only because the command refuses it in every chat.

Where no app takes the ask (an older app, or none), a sandboxed chat is refused as before and
sent to the window; outside the sandbox the command runs here, never forced.

**Not yet**: a folder another live chat holds (one live writer per branch folder, #3901, PR #3962) is
not refused by this removal until that lands; the app's `git worktree remove` then refuses a
folder whose files another process holds no more than git itself does.
