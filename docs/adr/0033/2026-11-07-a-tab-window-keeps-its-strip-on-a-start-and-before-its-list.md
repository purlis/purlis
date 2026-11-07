# Amended (2026-11-07): a tab window keeps its strip on a start, a resume, and before its list (#5533)

Closes the two cases [the amendment of 2026-11-06](2026-11-06-a-tab-window-keeps-its-strip-on-a-press.md)
left out. The rule is unchanged: a tab window's strip never changes for another strip's chat.

- **A chat started or resumed in a tab window is handed on as a press is.** A Fix, a theme or
  skill chat, a curation, a resume or a reopened task can be filed on a strip the tab window
  isn't showing. When it is, and no pane here shows it, the tab window changes nothing, and the
  project window comes forward. The tab window sends `press-here` with where the chat is filed
  (`filed`) and whether it was started (`started`), because the project window's list doesn't
  have it yet. The project window then opens it on that strip by the rule it would have used
  itself: a start replaces nothing, and a resume lands as a press. A start that names its own
  pane (`asked`) is left alone, as a breadcrumb's press is. If no window holds the project, the
  chat lands in the tab window after all.
- **A press before the tab window has read its list goes to the project window,** unless a tab
  here already holds the chat. Waiting for the list was the other choice. It was not taken,
  because a press that waits on a read that never answers would be lost, and the project window
  holds every strip, so it is right whatever strip the chat turns out to be on. The cost is that
  a chat of this strip, pressed in the moment before the list is read, opens in the project
  window instead.
