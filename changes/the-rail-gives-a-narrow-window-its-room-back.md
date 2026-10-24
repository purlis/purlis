### Changed

- **The project rail no longer crowds a narrow window.** At the narrowest window, or at the
  default window with both sidebars open, the rail hides itself so the work keeps its room. The
  window's left edge still peeks it, and the title bar's first button still shows it. It comes
  back when you widen the window or put a sidebar away. This hide is never written to the layout
  file. A rail you hid stays hidden. A rail you show or keep shown stays shown, in any window,
  until you hide it again (#3077).
- **New windows open at 1440×900**, or at your screen's usable size where that is smaller, so
  the project rail is shown beside both sidebars on a first launch (#3077).
