### Changed

- **A chat in a branch folder leaves the clone's git to the app.** A Claude Code chat started in a
  branch folder purlis cut no longer writes its clone's `.git`, as a chat above the folder already
  did: `git fetch`, `rebase`, `reset`, `stash`, `add` and `commit` stop there, and read-only git
  works as before. `purlis worktree commit`, `update` and `push` do those, and the new `purlis
  worktree update --fetch-only` fetches and moves nothing else, also with uncommitted changes.
- **A rebase stopped in a clone goes on or back by the chat's own git.** A sandboxed chat is told
  the step, `git -c core.editor=: rebase --continue` or `git rebase --abort`, and `purlis worktree
  update --continue` or `--abort` then puts the branch where it belongs. A branch folder and your
  own terminal work as before.
- **The app keeps its own record of an update in progress**, under the project's state, not in a
  git directory. A rebase stopped before you updated purlis, or a rewritten branch not yet pushed,
  is finished or pushed from your own terminal.
- **`purlis worktree push` asks the remote what it has**, and scans what the push sends against
  that, not against the clone's remote-tracking branches. The app's git reads commits as they are,
  without `refs/replace/`.
