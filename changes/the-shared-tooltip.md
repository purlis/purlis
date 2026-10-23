### Changed

- **Every hint is one tooltip, drawn in the theme and reached by the keyboard.** The window's
  hints were the platform's own tooltips, which a keyboard could not open and no theme could
  draw. Each is now the shared tooltip: beside what it is about, opened by the pointer after a
  moment or at once by Tab, taken down by Esc, and readable with the pointer on it. An icon
  button's tooltip is its name, and a disabled button whose hint says why stays reachable so the
  reason can be read (#2908).
