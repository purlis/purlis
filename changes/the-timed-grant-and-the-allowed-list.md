### Added

- **Allow for this chat, for 15 minutes or an hour.** At an ask you can now allow a chat for a
  while, with a one-line reason. The grant is the chat's alone (a chat it dispatches does not get
  it), ends at its time or when the chat closes, and leaves one quiet line in the Journal. A rule's
  `@timed` and an organisation's or project's `timed` lock cap it or switch it off, and an ask
  whose `@keep` is `once` offers none. At a deny there is no Allow: the way out is "Propose a
  project change…". The Allowed list shows each grant and timed grant with its reason, a countdown
  and Remove (#7030).
