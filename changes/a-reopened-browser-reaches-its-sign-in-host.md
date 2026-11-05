### Fixed

- **A browser reopened from kept logins reaches its sign-in host from the start.** For a login
  whose form is on another host, the reopened browser reaches that host as one logged in afresh
  does, so the site's own redirects, hidden frames and token refreshes work before the chat calls
  `browser_login`, and the chat stays kept off that host's pages outside a sign-in (#5167).
