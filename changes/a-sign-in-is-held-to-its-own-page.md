### Changed

- **A sign-in lets the chat act on its provider's site only in the sign-in's own window.** While
  `browser_login` signs in on another site, the chat reads and acts there on the page the login
  ran on and the windows it opens; another tab stays kept off that site.
- **A browser holding a login takes a secret only for the login's own sites.** A chat's fill on
  any other site is refused before anything is filled, and the chat is told to use a browser of
  its own, where it once left the browser reaching nothing.
- **A sign-in that asks for a second secret on its provider's site finishes by itself.** Once
  that site's server sends the page back to the login's own site, the page goes there without
  asking, while the sign-in is open.
- **A file a blank or embedded frame made is said to come from the site around it.** In a browser
  that signs in on another site, one from a frame with no site of its own is brought into the
  chat's folder only during a sign-in.
