### Fixed

- **The Chats list holds still while the keyboard is in a row's menu.** Its rows used to move
  under an open context menu. A list now also lets go as soon as the focus goes to a menu that is
  not one of its rows', or the pointer moves on after a row's menu closed under it. The Chats
  list and the Personas view use the same rule for this (#3594).
- **A screen reader no longer meets the rolled-up hand as a stray button in the Chats tree.** The
  hand stays a pointer target beside the row. The row already says which chat below needs you
  (#3658).
