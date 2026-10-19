### Added

- **Browser logins stay signed in across chats of a profile.** A chat's browser that logged in
  with `browser_login` keeps its logins once it ends, sealed on this device under its profile, and
  the next chat of that profile whose login is allowed starts signed in. A second chat at the same
  time gets a copy. Removing the profile removes them (#2021).
