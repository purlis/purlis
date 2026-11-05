### Added

- **`purlis worktree prune` clears a workspace's finished branch folders.** It removes every
  folder that is finished, task folders and plain ones, in every repo or the one you name: no
  uncommitted changes, every commit on its base or squash-landed, its task ended, no open chat
  holding it, and no other folder linking into it. A folder that links into another goes first.
  Each removed folder's branch is deleted unless you pass `--keep-branches`, and `--dry-run`
  lists what would go and why each other folder stays. A chat can run it too (ADR 0110, #5168).
