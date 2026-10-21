### Fixed

- **Typing into a large page while you drive stays quick.** After the first key into a field,
  a chat's browser no longer reads the whole page before each key: it asks the page only
  whether the same field still has the focus, and reads the page again when the focus moves.
  What you type stays out of the agent's answers as before (#2295).
