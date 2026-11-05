### Changed

- **Every browser login starts clean on its own site.** A login that fills first clears what that site
  stored in the browser: cookies, service workers, local storage, IndexedDB and cache, however the
  login was allowed. Other sites are left as they are, and a browser that already holds the login
  answers "already logged in" and clears nothing. If the login form doesn't appear after that, purlis
  opens a clean new browser for the login, says why in the answer and in the browser's timeline, and
  offers to close the one left behind (#5272).

### Fixed

- **A login follows the same value rules as a fill** (#5272).
