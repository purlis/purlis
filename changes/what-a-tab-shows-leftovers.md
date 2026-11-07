### Fixed

- **Closing a tab behind the one in front leaves the keyboard where it was.** Its × no longer
  takes the keyboard on its way out, so it stays in the front tab rather than landing on the
  window (#5445).
- **A narrow pane's head draws a chat's state once.** Under about 470px the head drops the
  state's word, and no longer draws its glyph a second time beside the chat's mark, which wears
  it already (#5446).
- **A folded pane that needs you shows one ✋.** Its rail drew its own ✋ beside the chat's
  mark, which wears one already; now the mark's is the only one (#5447).
