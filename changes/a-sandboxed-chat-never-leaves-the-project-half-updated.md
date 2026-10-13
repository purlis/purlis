### Fixed

- **A sandboxed chat can no longer leave the project half-updated.** A chat's sandbox never
  lets it write the project's manifest or the folders a later program loads code from, and git
  writes a tree file by file, skipping what it may not. So `git rebase origin/main` or `git
  pull` from a sandboxed chat at the project's root left most files as the remote has them and
  the rest, and the current commit, as they were, with nothing to say an update was tried. A
  chat's git that rewrites the root's working tree from another commit (`pull`, `rebase`,
  `merge`, `cherry-pick`, `revert`, `am`, `stash pop` and `apply`, `bisect`, a hard reset to
  another commit, a checkout of a branch the root is not on) is now refused before it starts,
  and the refusal says to ask the person to update the project. `purlis save` and `purlis save
  --pull` from such a chat bring nothing in where they would stop partway, and say which files
  are in the way; the window's own save is unchanged.
- **The way back from a half-updated project is open.** Restoring files to the current commit
  (`git restore --source=HEAD --worktree -- <paths>`, `git checkout HEAD -- <paths>`, `git
  reset --hard HEAD`, `--abort` of a rebase or merge) is never refused by that check. One that
  names a file the sandbox denies is refused up front and prints the `git restore` command for
  the person's terminal. `git checkout HEAD` with its paths in a file or from `xargs`, which
  the guard reads as a branch move, now says so and names the `git restore` spelling that is
  allowed.
