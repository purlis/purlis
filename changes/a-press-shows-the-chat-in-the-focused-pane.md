### Changed

- **Pressing a chat shows it in the pane you're in.** A sidebar row, a task, a Go or a name in a
  breadcrumb replaces what the focused pane shows, and the chat it replaces keeps running. A
  chat already on screen is jumped to rather than shown twice, a view is never replaced (the
  pane you used last takes the chat instead), and a chat from another workspace brings that
  workspace forward. The sidebar highlights the focused pane's chat and faintly marks the chats
  in the tab's other panes (#4757).

### Added

- **⌘⇧H (Ctrl+Shift+H) goes back to the pane's previous chat**, and again to come back. Where
  that chat is on screen in another pane, it goes there instead (#4757).
