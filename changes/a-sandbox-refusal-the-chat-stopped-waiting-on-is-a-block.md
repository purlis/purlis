### Changed

- **A connection the sandbox held is a block once the chat stops waiting on it.** While the chat
  waits on the held connection, it stays an ask in Asks with its Allow. When the hold times out,
  or the chat has moved on (the command it was held under has finished and the same agent makes
  its next call), the ask leaves Asks and the chat's event log gets one block from the sandbox,
  naming the host and the tool it was made under. A sub-agent's call or a parallel call does not
  count as moving on. The chat is told the connection is now a block and that running the
  command again asks again; it is no longer told that the ask stays with the person. A
  timed-out connection no longer stays in Asks as timed out, and the needs-you mark its ask
  raised goes with it (ADR 0124, R-290). A host the sandbox refused before it was named, then
  named on a held connection, is one ask, not two.
