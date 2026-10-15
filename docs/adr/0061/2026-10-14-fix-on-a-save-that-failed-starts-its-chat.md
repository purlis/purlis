# Amended 2026-10-14: Fix on a save that failed starts its chat on a message purlis sent (the operator's ruling P2, 2026-10-13)

Ruled: the window's **Fix** on a save's red request starts a chat **at once**. The chat's first
message is purlis's own, and it is sent: handed to the harness the way that harness takes a
first message, as a handoff's brief is. This is the first chat this record's machinery opens
whose prompt is not typed and left for the person to send, and it is an exception for that
button alone (`curation::start_sent`, whose caller names why from a closed set, `SentFor`).

- **Only the person's press starts it.** The command is the window's alone; no chat, tool,
  hook or `purlis` word reaches it.
- **The message is purlis's own.** The core writes it (`purlis_core::savefix`). What a failed
  check says is quoted in it as data, under purlis's sentence that says so (ADR 0051, amended
  the same day).
- **Nothing is typed, so nothing is gated by the board.** The message starts the chat; there is
  no prompt of a harness's for an Enter to land on. A harness with no measured way of taking a
  first message refuses, and nothing starts.

A curation prompt is still typed and never sent, and so is every other prompt prepared by this
record. Another chat started on a sent message is a new amendment here, and a new `SentFor`.
