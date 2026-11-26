### Changed

- **Hibernate says why a chat cannot be hibernated in one set of words for each reason.** purlis
  now decides in one place whether a chat is rested, that is, whether it may be hibernated: at
  the end of a turn, on a harness that resumes by itself and reports its background work, with
  no background work or dispatched task still running, no report owed to the chat that
  dispatched it, and run on this machine. When Hibernate refuses a chat, its pane names the
  reason. A chat on a runner is refused now too (SC-4c, #7632, #1391).
