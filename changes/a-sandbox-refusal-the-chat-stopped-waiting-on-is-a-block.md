### Changed

- **A connection the sandbox held is a block once the chat stops waiting on it.** While the chat
  waits on the held connection, it stays an ask in Asks with its Allow. When the hold times out,
  or the chat makes its next call, the ask leaves Asks and the chat's event log gets one block
  from the sandbox, naming the host and the tool it was made under. A timed-out connection no
  longer stays in Asks as timed out (ADR 0124, R-290). A host the sandbox refused before it was
  named, then named on a held connection, is one ask, not two.
