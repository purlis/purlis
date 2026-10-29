# Amended (2026-10-29, #3831, #3862; extends "a branch folder is kept current by the app"): a chat's clone, and a clean project root, are kept current by the app, and the update asks no approval — ruled by the operator, 2026-10-29

**Ruled by the operator, 2026-10-29**, twice: purlis does the update for the chat, as it
already makes commits for it, with no prompt, refused only where it would overwrite the
person's uncommitted work (#3831). And a chat may have purlis update the project root for it,
only when the root is clean: the window's own Update, refused on any uncommitted change in the
root and where the update would write a file the chat's sandbox denies (#3862). The command's
shape was decided in implementation.

1. **`purlis worktree update`, standing in a workspace's clone** (`workspaces/<ws>/<repo>`,
   not a branch folder), is handed to the app as a brokered git action of its own kind. The
   chat must stand in that clone, in its workspace's folder or at the project root, and a place
   the app cannot read is refused. The app fetches the remote the clone's branch follows under
   the chat's internet access. It then fast-forwards the branch to its upstream, or rebases the
   branch's own commits onto it, on commit ids read once, as a branch folder's update does.
   `--continue` and `--abort` finish or undo a rebase it stopped on a conflict, and only one
   purlis started: a rebase the person started is theirs. `--base` is a branch folder's only.
   A clone's `.git` must be a real folder, never a link to one. There is no new command word:
   git calls a clone's own checkout a worktree too.
2. **`purlis worktree update`, at the project root**, from the chat whose place the root is,
   runs the window's own Update (qw348) for it, its own kind of ask again. As for the person,
   it fetches the project's target branch from the recorded remote. It then fast-forwards, or
   replays the project's own commits on top, and leaves the project exactly as it was where it
   cannot. It is refused on any uncommitted change, untracked files included, before anything
   is fetched. After the fetch, the move from HEAD to the fetched commit is judged by name for
   the files a chat's sandbox denies (section 5: a project manifest, or a file a program run
   later loads code from), and refused where it would write one. The person then updates with
   Update in the window; this refusal stays fail-closed. The move runs onto the commit id that
   was judged. The root's update takes no `--base`, `--continue` or `--abort`: it moves whole
   or is put back.
3. **No approval, from the command or its tool.** A Claude Code chat the app starts carries
   allow rules for `purlis worktree update` and for its tool twin, `worktree_update` on
   purlis's MCP server, on the brokered commit's terms: alone in its call, or refused with the
   shape to retry. The tool runs nothing where no app takes it: the server runs outside the
   chat's sandbox, where the app's judging would not be.
4. **Refused, with the reason**, for a clone or a branch folder:
   - where the folder holds uncommitted changes;
   - where git has an operation stopped in it;
   - where the move would write a path that is on disk and that the current commit does not
     track. An ignored file would be overwritten by git without a word, and an untracked one
     would stop git halfway.
   - where the move would write a file the chat's sandbox denies, judged as before (section 5).

   **Narrowed, not closed:** the untracked and ignored check, like the clone check before any
   git runs, reads the folder before git moves it. A file a chat writes between that check and
   git's checkout is not seen. It narrows that window and does not close it, as the pinned
   clone check notes for the clone itself.
5. **The refusals name the command.** The clone guard's refusals of a `pull` and of a move it
   cannot name, the root guard's refusal of an update, the project-git refusal from a folder
   below the root, and a branch folder's blocked git record now name `purlis worktree update`
   as the way to retry. A move that would write a file the chat's sandbox denies still sends
   the chat to the person.
