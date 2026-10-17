### Changed

- **A persona's browser login is performed by purlis in the chat's own browser.** A chat calls
  `browser_login` with the login's name; purlis fills it from the vault on the declared origin
  alone, submits it and hands the browser back logged in, and that browser then reaches the
  login's site alone. The values never reach the chat, and no session file is written. Running
  the login as a command now does nothing and names `browser_login` (#2016).
