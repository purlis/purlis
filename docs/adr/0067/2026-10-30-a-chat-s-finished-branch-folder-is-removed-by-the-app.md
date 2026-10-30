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
   reaches (but see 5), and a folder purlis cut for a task is refused as it is from a
   terminal.
   `--delete-branch` deletes the branch as `git branch -d` does, only where it is merged.
   `--force` from any chat is refused by the command before anything is asked, naming the
   window's *Remove folder*, since the person decides what is discarded.
3. **Only a branch folder of the chat's own workspace, from above it.** The workspace must be
   one the chat writes (it stands in it, or at the project root), and the chat must not stand
   in another branch folder, nor in the folder itself, whose removal would take the chat's own
   folder from under it; a place the app cannot read is refused. The chat that cut the folder,
   or any chat opened in the workspace's folder, removes it from there. The ask names a repo and a piece, so it reaches only
   `<workspace>/.worktrees/<repo>/<piece>`: never the clone itself, nor the project root.
4. **No approval.** A Claude Code chat carries `Bash(purlis worktree remove *)` among its
   own-scope rules (ADR 0064 as amended for #3829), alone in its call as a commit is (a rider
   is refused), and the `worktree_remove` tool joins the own-scope tools. The rule matches
   `--force` too, which is safe only because the command refuses it in every chat.

Where no app takes the ask (an older app, or none), a sandboxed chat is refused as before and
sent to the window; outside the sandbox the command runs here, never forced.

5. **Landed by its files.** A folder whose branch landed by a squash merge (on review,
   2026-10-30) holds commits no other ref reaches once the forge deletes the branch and the
   clone's fetch prunes it, yet nothing in it is lost. Its branch is asked of the base as
   `dispatchplace::tidy` asks it (`standing::Landed::Carried`: every file it changed reads in
   the base as it has it), and of the base's remote branch too, which the clone's fetch moved
   while its own base stayed behind. Such a folder goes without `--force`
   (`worktree::remove_keeping_branch`), its branch kept as the one place its commits are, and
   uncommitted work still refuses it. The terminal's removal reads it the same way.
6. **One live writer** (#3901). The removal is held to the folder's claim as a commit, an update
   and a push are (`piececlaim::removing`): a folder another live chat holds is refused, in the
   same words, and so is a held folder where no app says whether its holder lives (the
   command's fallback in a chat). The person's own terminal removes any. A removal that
   succeeds writes a `removed` line to the piece log, which ends the folder's claim, whoever
   held it.
7. **`--force` is refused in the hook too**: purlis's PreToolUse guard refuses a Claude Code
   chat's `purlis worktree remove … --force` before it runs, outside the chat's sandbox, so the
   refusal does not rest on the chat's environment.
