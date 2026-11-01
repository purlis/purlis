### Fixed

- `purlis worktree update --continue` after an update with `--base` recorded the new base on the branch again, rather than writing an empty `[branch ""]` section that stopped every chat on the clone. A clone that already has that section recovers on its next `purlis worktree add`, `update` or `push`, and a push on a config git will not read says so rather than "has no remote to push".
