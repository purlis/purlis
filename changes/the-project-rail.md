### Changed

- **Projects are a rail at the window's left, and it can hide itself.** The project tabs in the
  title bar are gone. Each project is a button in a column at the far left, showing its two
  letters in its own colour, how many things wait on you there, a dot while it has unsaved work,
  and a ring on the one you are in. **+** opens a project. The rail's last button hides it; while it is hidden, the title
  bar's first button shows it again, and moving the pointer to the window's left edge slides it
  in over the work, where **Keep sidebar shown** pins it again. Hiding and showing slide, and
  don't under reduced motion. The layout file remembers which way you left it (`rail`). A project's menu
  (right-click, or the menu key) still closes, pins and moves it, Delete closes the focused one,
  and Shift+Space moves it up or down the rail (#2915).
- **Themes have three more motion durations**: `duration.slide`, `duration.reveal` and
  `duration.leave`, for the rail sliding, its peek coming in, and the peek going (#2915).
