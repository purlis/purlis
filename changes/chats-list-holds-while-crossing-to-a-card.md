### Fixed

- **The Chats list holds still while the pointer crosses from a row to its card.** The card stands
  a few pixels beside its row, and moving the pointer across that gap let the list go, so the rows
  could reorder and the card move with its row before the pointer reached it. While a card is up,
  the list now waits as long as the card does before it lets go (#3773).
