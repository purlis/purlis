### Added

- **purlis keeps what you answer in one sealed store, and notices when it is taken away or put
  back from before.** Every answer is sealed on its own, a sealed manifest lists them, and the
  keyring holds a counter that only rises, so a deleted, older or swapped copy of the store, a
  swapped `.git` or a moved project reads as a refusal rather than as the shipped default. Only
  a grant that does not hold counts as no answer. The deny state is decided here; what a chat is
  refused on it follows (#7015).
