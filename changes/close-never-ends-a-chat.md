### Changed

- **Closing never ends a chat.** Close pane clears a pane and Close tab closes a tab. Their chats
  keep running in the sidebar, nothing is asked, and the close button no longer turns red under
  the pointer. To end a chat, use End… in its list. Closing the last tab on a strip leaves one
  empty pane (#4756).
- **A chat whose program ended keeps its pane.** Its last screen stays, read-only, under a line
  that says it ended, with Resume to start it again on its conversation and Clear pane. The
  keyboard stays where it was (#4756). A task left in a pane as it ended does the same: the line
  says how it ended and shows its report, and Resume reopens it in that pane (#4757).
