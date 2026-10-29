# Amended (2026-10-29, #3831; extends "a branch folder is kept current by the app"): a workspace's clone is kept current by the app too, and the update asks no approval — ruled by the operator, 2026-10-29

**Ruled by the operator, 2026-10-29**: purlis does the update for the chat, as it already
makes commits for it: no prompt, refused only where it would overwrite the person's
uncommitted work. The command's shape was decided in implementation.

1. **`purlis worktree update`, standing in a workspace's clone** (`workspaces/<ws>/<repo>`,
   not a branch folder), is handed to the app as a brokered git action of its own kind, for a
   chat standing in that clone, in its workspace's folder or at the project root. The app
   fetches the remote the clone's branch follows under the chat's internet access, then
   fast-forwards the branch to its upstream, or rebases the branch's own commits onto it, on
   commit ids read once, as a branch folder's update does. `--continue` and `--abort` finish
   or undo a rebase it stopped on a conflict. `--base` is a branch folder's only. No new
   command word: git calls a clone's own checkout a worktree too.
2. **No approval, from the command or its tool.** A Claude Code chat the app starts carries
   allow rules for `purlis worktree update` and for its tool twin, `worktree_update` on
   purlis's MCP server, on the brokered commit's terms: alone in its call, or refused with the
   shape to retry. The tool runs nothing where no app takes it: the server runs outside the
   chat's sandbox, where the app's judging would not be.
3. **Refused, with the reason**, where the folder holds uncommitted changes, where git has an
   operation stopped in it, and, for a clone or a branch folder alike, where the move would
   write a path that is on disk and that the current commit does not track: an ignored file,
   which git would overwrite without a word, or an untracked one, which git would stop on
   halfway. The judging of section 5 stands as it was: a move that would write a file the
   chat's sandbox denies does not start, and names the file.
4. **Never the project's root folder.** The root is the person's checkout; updating it stays
   with the person (the window's Update, or their own terminal), and the root guards still
   send a chat there. The clone guard's refusals of a `pull` and of a move it cannot name, the
   project-git refusal from a folder below the root, and a branch folder's blocked git record
   now name `purlis worktree update` as the way to retry.
