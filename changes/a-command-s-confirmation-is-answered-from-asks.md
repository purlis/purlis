### Changed

- **A command's confirmation is answered from Asks, with no dialog over the window.** When
  `purlis profile sign-in --pool`, `purlis pool remove-profile` or `purlis signing check` waits
  on your yes, its row in Asks shows the code its terminal shows and its two answers. The ✋ and
  the rest of the window stay in reach while it waits, still for up to two minutes. With no
  project open in the window, the command now says to open one, rather than waiting (#6136).
