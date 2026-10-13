### Fixed

- **A note that names a variable no longer blocks the project's save.** A line such as
  `token = DEPLOY_TOKEN` or `` `password = KEYCLOAK_ADMIN_PASSWORD` `` says which environment
  variable or vault entry carries a credential, and holds none. The save read it as a
  credential and refused. A value that is only such a name (capitals, digits and underscores,
  in words, one of them a word like `TOKEN`, `SECRET`, `KEY` or `PASSWORD`) is now let through.
  A key written in capitals is still refused, and so is a real value after the name.

### Changed

- **A save blocked by what looks like a credential says where.** The title bar and the Inbox
  name the file and the line ("Blocked: a secret-shaped value in personas/…/note.md, line 5").
  The Saving view lists each file with its line, the kind of value, and what was matched with
  the value hidden, and offers **Open the file**, which starts your editor at that line, and
  **Save again**. The value itself is never shown.
