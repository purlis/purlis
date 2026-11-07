### Fixed

- **A browser login refused before it ran no longer spends its Allow once.** Where the browser had
  ended, or held another login's site, or was held by a page holding a filled secret, the chat's
  Allow once stays its own, and the browser's record of its session is kept, so the next call can
  still answer "already logged in" without reading the vault again (#5251).
