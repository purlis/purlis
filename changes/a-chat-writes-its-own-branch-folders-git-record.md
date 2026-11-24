### Fixed

- **A chat in a branch folder purlis cut can write that folder's own git record.** `git reset`,
  `merge` and `fetch` no longer fail on `index.lock` and `FETCH_HEAD` in the chat's own folder
  (#7550). Only the one record in the clone's `.git/worktrees/` that names the folder back is
  given; its `commondir`, `gitdir`, `config.worktree`, `hooks` and `info` stay held, and the
  clone's config, hooks, branch refs, objects and other branch folders' records stay read-only.
  Commits, and fetches that write objects, still go through `purlis worktree`.
