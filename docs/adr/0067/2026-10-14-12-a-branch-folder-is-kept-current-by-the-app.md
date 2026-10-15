# Amended (2026-10-14, qw387; amends the brokered commit's "It only commits"): a branch folder is kept current by the app — ruled by the operator, 2026-10-15

**Ruled by the operator, 2026-10-15.** Decided in implementation, on the operator's ask.

A branch folder's index, HEAD and rebase state are in its clone's `.git`, which section 5
denies a chat, so a sandboxed chat's own `git fetch`, `git rebase` and `git reset` fail there.
Two verbs beside `purlis worktree commit` are handed to the app as a brokered git action,
for a chat standing in that folder or above branch folders in its workspace, and run the
same core code a terminal runs:

1. **`purlis worktree update [--base <branch>] [--continue | --abort]`** fetches the base's
   remote under the chat's internet access, brings the base branch up to its remote-tracking
   branch where that is a fast-forward and no worktree has it checked out, and rebases the
   folder onto it; onto the remote-tracking branch itself where the base is checked out and
   behind it. A folder with uncommitted changes is refused. A move that would write a file
   section 5 denies the chat is judged by the clone guard's own rules and does not start,
   naming the file. A conflict stops it and names the files; `--continue` stages exactly
   those, refusing one that still holds a conflict marker, a submodule and a file marked for a
   content filter; `--abort` puts the branch back. The rebase runs with no editor, no signer,
   no hook and no other branch moved.
2. **`purlis worktree push [--force-with-lease]`** pushes the folder's own branch to its
   remote under its own name, never the branch the remote names its default, after purlis's
   scan of what it would publish. `--force-with-lease` only right after an update rewrote the
   branch, leased to what the remote had when it was fetched just before.
3. **`purlis worktree add` cuts from the branch the remote names its default** (`origin/HEAD`),
   or `--base <branch>`, brought up as in 1; `--base HEAD` keeps the clone's HEAD. An ask from
   an older command, which names no base, keeps the clone's HEAD.
4. **An app-run checkout is judged as a chat's own would be.** For a sandboxed chat the app
   fetches before it cuts, so the remote-tracking branches it reads are its own fetch's, and a
   cut that would write a file section 5 denies the chat, where the clone has that file
   otherwise or not at all, is refused and named; the person cuts it from their own terminal.
   So is the worktree a dispatch cuts for a task from the asking chat's branch: the dispatch
   is refused, and the asking chat is told which file. Every such checkout runs on commit ids
   read once: what is judged is what git checks out, and a branch is moved only from the
   commit it was judged at. A lease is given only on purlis's own record of its last update,
   and the remote's default branch is asked of the remote.

The brokered commit's "It only commits" stands for `purlis worktree commit` itself.
