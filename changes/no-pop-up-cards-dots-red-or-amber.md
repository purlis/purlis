### Changed

- **Nothing pops up by itself any more.** A result about a workspace, or about the project or the
  app, no longer brings a card up over the window. It waits behind the dot on its workspace's chip,
  or on the status line's Notices item, with a "new" pip until you have seen it, and is listed in
  Notices as New. Pressing the dot still shows its card (ADR 0121 decisions 1, 3 and 10; #6004).
- **The status line no longer repeats counts the right bar shows.** Notices has no number there,
  and the Doctor item has none while Notices lists the doctor's findings (#6004).
- **The Notices dot is red or amber.** It is red while a Notice says something is broken, amber
  while one asks for a decision that stops no chat, and absent otherwise (#6004).
