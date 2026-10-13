### Fixed

- **Switching back to a project builds nothing for the four projects you were in last.** A
  project you were just in keeps its view and its chats' terminals while another is in front, so
  going back to it puts it on screen as you left it instead of building it again; what it showed
  is read again behind it. A project you have not been in for longer, or one you left with a
  dialog open, is drawn again when you come back to it, as before.

### Added

- **The doctor's dialog lists what each project switch in this window took**: how long to the
  window having drawn it and to that being painted, and how long the core took to answer each
  command it answers on the window's own thread. Kept in memory for the window, and yours to
  copy.
