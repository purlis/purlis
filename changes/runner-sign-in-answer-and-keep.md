### Fixed

- **A runner's sign-in answer goes only to a socket that login's own programs hold.** Where the
  runner's system can't say who holds a socket, it goes where the login was found listening, as
  before.
- **A runner's keep also clears Codex's `.env`**, which no desktop sends.
- **A page's listed title is left empty when it is as long as a title can be and the page keeps a
  text**, so a cut can't leave part of that text in the list.
