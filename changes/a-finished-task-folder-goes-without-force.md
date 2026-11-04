### Changed

- **A finished task folder can be removed without the Changes tab.** `purlis worktree remove`,
  from a chat or a terminal, and the explorer's Remove now take a branch folder purlis cut for a
  task once it is finished: its task has ended, nothing in it would be lost, and no other folder
  links into it. `--delete-branch` goes with it. A task folder that is not finished is still sent
  to its Changes tab, and the refusal now says why (ADR 0110, #5168).
