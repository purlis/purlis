### Fixed

- `purlis worktree update --base <branch>` on a branch folder cut from `origin/main` while the clone's `main` was behind it no longer carries main's newer commits onto the new base: a folder with no commits of its own lands exactly on that branch's head, and one with commits replays only those (#7875).
