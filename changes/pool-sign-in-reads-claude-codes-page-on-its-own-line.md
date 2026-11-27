### Fixed

- **Signing a Claude Code profile in on a pool no longer hangs at "Opening the sign-in page".**
  Claude Code now prints its sign-in page on a line of its own, and purlis read it only on the
  same line as the words before it, so no browser opened. purlis now reads both. While the strip
  waits on the runner, it says so after 30 s, with when purlis stops waiting at that step, and
  offers Try again. The log names each step of a sign-in: `profile_sign_in_start`,
  `profile_sign_in_address` and `profile_sign_in_done`.
