### Added

- **Saving a theme folder repaints the window** (#2324). Edit a theme in use — your own, the
  project's or a workspace's — and the window draws it as soon as you save, with no new window.
  A burst of saves is one repaint. A save that is not a theme draws Purlis, with a Notice that
  says what is wrong, until it is fixed. A project's font file that changed since you approved
  the project is drawn once you approve it again, and a Notice says so.
- **An extension that brings only themes needs no approval** (#2324, ADR 0098). Installing one
  asks nothing, editing its themes asks nothing, and the Extensions list says it is themes only.
  An extension with commands, panels, views or a program is approved as before.
