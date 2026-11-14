### Changed

- **A merge a chat's hook held is asked on that chat's tab too, as well as in Asks.** Merge
  and Don't merge are on the chat's own pane, Merge asks once more there as it does in Asks,
  and an answer in one place clears both. A held merge is a risk of its own, Merge, and its row
  in Asks is drawn as one. Two chats holding the same merge are two rows, never answered once
  for all, and nothing on a held merge is ever kept as a rule (#6430).

### Fixed

- **⌘⇧I no longer lands on a high-risk ask that stands first in Asks.** From a chat's terminal,
  triage put the keyboard past it, then the list took it back to that first row a moment later.
  It now stays on the first ask triage may land on, and the keyboard stays wherever a script in
  the window moves it inside a list (#6430).
