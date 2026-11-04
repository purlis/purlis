### Changed

- **New keys for panes and tabs.** ⌘⌥ and an arrow moves to the pane in that direction (Alt and
  an arrow off a Mac, once Ctrl+Tab has left a chat's terminal). ⌘⇧] and ⌘⇧[ go to the next and
  previous tab (Ctrl+Shift+] and [ off a Mac). ⌘⇧J shows the focused chat's tasks, and ⌘⇧H goes
  back to the chat the pane showed before. The keys that went to the next and previous chat in a
  tab are gone, since a chat isn't a tab (#4767).
- **Closing a pane gives the keyboard to the pane that takes its space**, not to the tab's first
  pane. Ctrl+Tab out of a pane's terminal lands on the next pane's controls. A screen reader
  hears each pane of a split by where it is ("Left pane"), and hears one line when a chat you
  pressed is already in another pane and the keyboard moved there (#4767).
