### Changed

- **A chat can push the branch of a task it asked for only when that task ended, ran the same
  persona in the same workspace, and the branch is the one purlis cut for it.** A task that still
  runs (or a handoff of the same chat, or a record purlis cannot read), a grandchild task, another
  persona or workspace, or any other branch the task made is refused, and the person is asked to
  let the chat push it. The follow-up is on unless the project sets `[policy]` `lineage = "deny"`
  or an administrator's policy sets `sandbox.lineage` to `false`; a value purlis cannot read means
  no follow-up, and allowing it again waits for your press. The branch a chat owns by your Allow
  is never taken up by the chat that asked for it, however often it is pushed again. A project
  keeps its integration branches out of every chat's push by naming them, exactly or with `*`,
  in `[risk] protected_branches` (read from the last commit, and never fewer than you accepted);
  `integration/*` is protected with no setting, and ordinary task branches named like
  `m75-20-grants-…` are not taken for integration branches.
