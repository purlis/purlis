### Added

- **F6 moves the keyboard from region to region**: left sidebar, editor, right sidebar, the
  Panel's left half, its right half, skipping what is put away; Shift+F6 goes back. Inside a
  chat's terminal F6 is still the chat's (spec #1736, #1741).

### Changed

- **A narrow window draws one half of the Panel at a time**: below 900 px wide, the half used
  last, and the other side's Lower icons switch to theirs. Widen the window and both come back
  as they were (#1741).
- **Each half of the Panel is one Tab stop and a named region** for screen readers; the chevron
  on its line is no longer a Tab stop of its own (#1741).
