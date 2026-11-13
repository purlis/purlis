# Amendment, 2026-11-12: a close waits a second for its host, then hears it off the window's thread

Of the `INPUT_ORDER` commands, this record says "Its own call to the host still waits as it did
before this decision", up to the 30 s a quick command waits. For a chat's close
(`close_session`) that wait is now bounded too (#6227). On a macOS runner a session host took
all 30 s to answer a Close. The app's watch logged a 29.9 s stall of the main thread running
`close_session`, and the window, and the WebDriver inside it, froze for as long.

- **A close waits for the host's answer for 1 s at most on the thread that asked**
  (`hostedsessions::A_CLOSE_WAITS_AT_MOST`, the lane's `A_TURN_WAITS_AT_MOST`). After that it is
  waited for on the link's runtime, as long as any call to that host is (`ANSWER_WITHIN`, or a
  pool's answer wait), and a refusal or no answer is logged as before. The answer changed
  nothing here before either: the chat is off the window's board and out of `running` before the
  host is asked, and `close` answered `Ok` whatever the host said.
- **The command stays synchronous**, so it still lands after what was typed to the chat.
- **What this gives up:** "a close before the resume that takes its place" held because the
  window waited for the close's answer. A host answers a Close at once, before its program ends,
  so the bound is reached only when the host is not answering. Then a start under the same
  number that reaches the host while it still holds the old program is given another number by
  the host, and the window refuses a start numbered otherwise than it asked and closes it, as it
  always has. A refused start is the cost, where it was a frozen window.

The rest of this record stands.
