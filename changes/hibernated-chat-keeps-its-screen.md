### Added

- **A hibernated chat keeps a copy of its screen.** When you hibernate a chat, purlis keeps a copy
  of what it showed, its screen and scrollback, and opening the chat shows that copy read-only
  under Resume. The copy is at most 1 MiB, the newest lines kept, and any secret purlis handed
  the chat is masked in it. It is deleted when the chat is closed, once it has been resumed and
  has drawn again, and after 30 days. Where no copy could be kept the pane says so, and Resume still works (SC-4b,
  #7631, #1391).
