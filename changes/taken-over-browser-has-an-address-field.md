### Fixed

- A chat's browser it asked you to take over (`ask_operator` with a browser) now has an address
  field above its page, so you can open an address the chat gave you. It goes only where the
  chat's own `browser_navigate` could go: a site new to the project is held back and asked about
  under the field, as the chat's own sites ask, and the browser stays yours until you press Done
  (#6290).
