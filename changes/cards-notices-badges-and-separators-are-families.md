### Changed

- **Cards, Notices, badges, counts and separators each have one look.** An ask in the Inbox, a
  task's question in a chat and a block's report under its Notice are one card: in Purlis on the
  window's fill with corners 10 and no hairline, and an ask keeps its waiting edge in every
  theme. Every badge and count is a pill in 11px words, and Settings › Sandbox › Your hosts says
  each host's status: "Waiting for you" on the amber wash until you confirm it, then
  "Confirmed". The line between two groups (a menu's sections, a dialog's footer, the rows of the
  Inbox's Notices) is its own part, as thick as the theme's `separator.width`, 0 for none, and in
  Purlis a plain or primary button and a field draw no border at any size; Allow and a
  destructive answer keep theirs. A theme sets these
  through `card.*`, `notice.*`, `badge.*`, `count.*` and `separator.*` parts, and `purlis theme
  parts` lists them; a card's or a Notice's fill is held to a ground an Allow on it is measured on
  (#2912).
