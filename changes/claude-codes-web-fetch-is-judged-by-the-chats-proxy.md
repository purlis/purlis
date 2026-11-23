### Changed

- **A sandboxed Claude Code chat can fetch a page from a host you have allowed.** At Full, its
  `WebFetch` is no longer switched off: purlis asks the chat's own proxy about the page's host,
  which carries an allowed host, asks you about an unknown one, and refuses a blocked one with a
  block you can see. `WebSearch` is allowed and recorded (T1.1-39, #6886).
