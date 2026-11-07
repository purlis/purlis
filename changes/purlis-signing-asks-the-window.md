### Changed

- **`purlis signing` no longer needs GitHub's `gh`.** `setup`, `status` and `check` ask the
  running purlis window, which asks GitHub with its own sign-in. In the terminal you add the key
  on GitHub's page by hand, and the window checks that GitHub has it. With no window running,
  `setup` gives the steps and says to open purlis. `setup --by-hand` is gone, since adding by
  hand is now the only way from the terminal (#5625).
