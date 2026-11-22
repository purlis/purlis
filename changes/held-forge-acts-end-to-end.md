### Added

- **A tag, release, fork or CI-file push a chat asks for now waits in Asks, and your press makes it.** A chat holds no forge credential, so when it asks for one of these purlis does not refuse it dead. Asks shows what will be made, read from the forge and your clone and not from the chat: a tag's commit and whether it is on the default branch, a release's notes, the changed lines of a CI file with their paths, or the account a fork goes to. It takes two presses, each naming what you were shown, and there is a Don't beside them. The app makes it with your own sign-in, and a CI-file push on GitHub with the second sign-in that grants the workflow permission.

### Security

- **The changed lines of a held CI-file push are read from objects purlis hashed itself.** The diff comes from a quarantine the app owns, holding the base from the forge and the pack the chat pushed, never from the chat's clone, whose objects a chat can overwrite. On GitLab a push that adds, changes, deletes or renames any `.yml` or `.yaml` file, or the project's configured pipeline file, is held, decided by the file's name and never by reading a pipeline file (so a YAML-only edit that nothing includes now asks too); on GitHub anything under `.github/` and any `action.yml` is held; a tag name with a character that draws nothing is refused, and a CI path with a line break is not offered.
