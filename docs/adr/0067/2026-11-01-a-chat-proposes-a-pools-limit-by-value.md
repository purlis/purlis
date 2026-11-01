# Amended (2026-11-01, #4284; the person's ruling on PC-1, spec #4275's Q6): a chat proposes a pool's limit as a value, never as an edit of the file

Ruled by the person. PC-1 keeps a proposed change to the plane's own configuration, and never
purlis's state. A pool's limits live in `<config>/runners.json`, which also pins its runners, so
that file stays unproposable. A chat proposes a limit this one way:

- **It carries a value and no file.** In a chat, `purlis pool limit <pool> <key> <value>` (or
  `--reset`) hands the app `Ask::ProposeLimit`: the pool, the limit's key and the value. It writes
  nothing, and a chat that can't reach the app is refused rather than falling back to the file.
- **The same ask.** It is held with the other proposals, on the chat's tab and in the Inbox, with
  Apply and Dismiss, showing the limit's plain name and its old and new value. One a limit a chat
  (PC-9). No "always" (PC-13). Policy's `sandbox.proposed-changes = "off"` holds none (PC-14).
- **The app judges it.** The list of limits checks it when it's asked for and again at Apply. A
  value changed since makes it stale (PC-8). Apply is audited as any other (PC-4), and saves
  through the one function the window's own page uses, so only that value can change.
