### Security

- **A signed-in browser is handed over only to a chat with that very login** (#2699). Handing a
  browser that holds a login, or one opened from a kept login, to another chat now checks the
  login's whole identity, its persona, project and declaration, not only its name, for every
  login the browser was signed in with: a chat whose login of the same name is declared
  otherwise is refused, and the browser stays with no chat.
