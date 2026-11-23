### Added

- **`purlis inbox` lists what is waiting in needs-you, and denies or dismisses it.** An ask that
  reaches nobody (the chat has nobody at it, no window can be reached, or nobody answered before
  the wait's cap) is refused, and raised once per rule, resource, workspace and request. `purlis
  inbox list` shows each with how many chats asked and when last; `deny` tells the chats at their
  next call, `dismiss` tells nobody. There is no allow: allowing is the window's alone (M75, #7028).

### Changed

- **An ask nobody answers is refused before the harness's own prompt would decide it.** The wait
  is capped below the harness's hook timeout, and the chat is told it was refused and that the
  ask is in needs-you.

### Removed

- **purlis no longer offers to save a rule after you give the same Allow twice.** A rule made
  earlier stays in Rules I made, where Undo takes it away (M75, Q314.15, #7028).
