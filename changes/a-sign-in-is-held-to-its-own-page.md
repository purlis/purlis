### Changed

- **A browser holding a login takes a secret only for the login's own sites.** A chat's fill on
  any other site is refused before anything is filled, and the chat is told to use a browser of
  its own, where it once left the browser reaching nothing.
- **A sign-in that asks for a second secret on its provider's site finishes by itself.** Once
  that site's server sends the page back to the login's own site, the page goes there without
  asking, while the sign-in is open.
