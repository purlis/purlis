### Fixed

- **On macOS the window stops asking once the command that asked has ended.** When you stopped
  `purlis profile sign-in --pool` while the window asked you to confirm it, the window kept
  asking until its two minutes ran out, because macOS refuses the way purlis checked whether the
  command was still there. It now closes the question once the command is gone, as it does
  on Linux.
