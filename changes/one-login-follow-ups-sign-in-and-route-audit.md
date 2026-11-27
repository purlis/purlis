### Added

- The doctor has a `forge blocks` row. It lists the newest requests that the forge routes
  of this project's running chats blocked, with the chat's name and the reason in plain words, so
  you can read why `gh` or `git` was blocked in a chat. It shows no token and no path, and a host
  name that a chat sent shows only when it is a plain host name.

### Changed

- A chat's new connection waits at most three seconds for a forge's name to resolve, so a forge name
  that does not answer no longer stalls it for the system's own timeout.

### Fixed

- Asking GitHub for the CI-file permission, then cancelling or quitting, leaves GitHub's code usable
  until it expires. A sign-in made in that time now records what it may do as not known, and your
  chats do not use the account until the code has lapsed and a sign-in or a retry has read it.
- The messages after a CI-file permission request no longer say "nothing changed" when your chats
  have already lost the account. They say that chats cannot use it until you try again or sign in
  again, or, while an earlier code is still live, how long to wait for it to expire.
- Asking for the CI-file permission twice no longer forgets the first code: each code is recorded
  and ended on its own, so approving, refusing or losing the second one cannot make your chats
  trust the account while the first can still be approved.
- A GraphQL request that names several of a chat's repos is now judged by the account that serves
  each repo, not by the token. A token renewed between two repos no longer refuses it.
