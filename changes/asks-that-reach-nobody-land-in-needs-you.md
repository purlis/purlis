### Added

- **`purlis inbox` lists what is waiting in needs-you, and denies or dismisses it.** `purlis inbox
  list` shows each item (one per rule, resource, workspace and request) with how many chats asked
  and when last; `deny` tells the chats at their next call, `dismiss` tells nobody. There is no
  allow: allowing is the window's alone (M75, #7028).

### Removed

- **purlis no longer offers to save a rule after you give the same Allow twice.** A rule made
  earlier stays in Rules I made, where Undo takes it away (M75, Q314.15, #7028).
