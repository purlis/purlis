### Changed

- **A chat can push the branch of a task it asked for only when that task ended, ran the same
  persona in the same workspace, and the branch is the one purlis cut for it.** A task that still
  runs, a grandchild task, another persona or workspace, or any other branch the task made is
  refused, and the person is asked to let the chat push it. Integration branches (`phase-*`,
  `integration/*`, a milestone's `m75-*` and any name in `[risk] protected_branches`) are never
  pushed through a chat's forge route, whoever made them. Setting `[policy]` `guards.forge` to `off`
  switches the follow-up off.
