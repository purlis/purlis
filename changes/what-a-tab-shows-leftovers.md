### Fixed

- **Closing a tab behind the one in front leaves the keyboard where it was.** Its × no longer
  takes the keyboard on its way out, so it stays in the front tab rather than landing on the
  window (#5445).
- **A narrow pane's head draws a chat's state once.** Under about 470px the head drops the
  state's word, and no longer draws its glyph a second time beside the chat's mark, which wears
  it already (#5446).
- **A folded pane that needs you shows one ✋.** Its rail drew its own ✋ beside the chat's
  mark, which wears one already; now the mark's is the only one (#5447).
- **In Classic, the × of a tab under the pointer stands on the tab's own hover colour,** not on a
  lighter box (#5524).

### Changed

- **A chat's skill run and its browsers are on its pane head, not its tab.** A tab says which
  chat it is and whether it needs you; what the chat uses now is beside its state on its pane's
  head, and each browser is still a row under its chat (#5524).
- **A head with no room for its state's word shows its tasks' button as a list,** not a second
  copy of the state's glyph beside the chat's mark (#5524).
