### Changed

- **A timed-out ask stays answerable.** A permission prompt whose hook its harness gave up on, a
  held connection, a run grant or a secret no longer vanishes when its time runs out: it stays
  in Asks after the live Expiring asks, saying "Timed out. Answer, and the chat is told to try
  again." Answering a permission prompt late answers the chat's same request at once when it
  makes it again, on Claude Code, Codex and opencode, and the chat is told on its next turn.
  While you're Quiet, a held connection waits on past its minute instead of timing out (ADR
  0121, #6007).
