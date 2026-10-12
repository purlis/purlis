### Changed

- **A memory or a session record saved in the workspace in front redraws only the panel that
  lists it.** The focused workspace's panels are held in a store of their own, and each region
  takes the part it draws, so a memory an agent saves no longer redraws the whole project
  window, and a session record redraws it once, for the palette's rows, where it was twice. A
  memory's or a todo's tab is read again only for a write to its own store, not for every todo
  or memory written anywhere in the project (#1160).
