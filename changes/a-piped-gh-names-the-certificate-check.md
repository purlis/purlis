### Fixed

- **A certificate check refused down a pipe gets its own Notice.** When a sandboxed chat ran
  `gh pr list … 2>&1 | head` and the sandbox refused `gh` the system's certificate check, the
  Notice said "The sandbox did not say which system service it refused" and offered only
  *Start without the sandbox*. The pipe ended with `head`'s status, so the command did not
  count as failed, and the service was named only from a failed command. The certificate check
  is one service, and its name is purlis's own, never taken from what the chat printed. So a
  line that runs a program now names it however the line ended (a pipe, `|| true`,
  `; echo $?`, and the one text Codex and opencode hand over), and the Notice says what stands
  for certificate checks in this project and the way on (they are on by default now, so there
  is nothing to allow). A line that only reads (`cat` of a log, `grep`, `git show`, a here-document's body)
  and a file a chat read still name no service.
