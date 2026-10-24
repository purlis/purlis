### Changed

- **The project rail no longer crowds a narrow window.** At the narrowest window, or at the
  default window with both sidebars open, the rail hides itself so the work keeps its room. The
  window's left edge still peeks it, and the title bar's first button still shows it. It comes
  back when you widen the window or put a sidebar away. This hide is never written to the layout
  file. A rail you hid stays hidden. A rail you show in a narrow window stays shown until you
  hide it again (#3077).
