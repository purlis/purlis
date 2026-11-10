### Changed

- **A sandbox consent and a command's confirmation are asks.** When a project expects a local
  socket or system service that this machine hasn't allowed, the request is now listed in Asks
  with **Allow on this machine** and **Not now** on its row. The ✋ counts it and you get a
  notification. Its Notice stays, and answering in either place settles both. A `purlis`
  command waiting for your confirmation in the window (a pool sign-in, taking a profile off a
  pool, a signing check) is listed in Asks too, with its code, still for up to two minutes.
  `purlis profile sign-in --pool` and `purlis pool remove-profile` now say where they wait:
  "waiting for the person: Asks (✋) in the purlis window, project …, for up to 2 minutes".
