### Changed

- **A browser login stays in the browser you're watching.** `browser_login` now fills the browser the
  chat names or is using, instead of opening a new one each time. A browser that already holds the
  login answers "already logged in" and fills nothing. A new browser opens only for a login kept for
  later reuse, and the chat and the browser's timeline say why and offer to close the one left behind
  (#5121, #5096).

### Security

- **A browser login follows the rules of any secret fill.** Its fields give nothing to a copy, a cut or
  a drag, the chat copies and drags nothing in that browser from then on, a user name goes only into a
  one-line field, and only the page purlis opened for the login is filled. A login you allow on its notice starts clean on
  its own site, and other sites in that browser are left as they are (#5121).
