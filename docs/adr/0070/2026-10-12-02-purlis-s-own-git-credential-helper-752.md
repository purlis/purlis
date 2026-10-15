# Built: purlis's own git credential helper (#752, 2026-10-12)

The helper this amendment bounds is written (`purlis_core::forge::gitcredential`):

- **It is the app's own binary, started again by git** with `purlis-git-credential`, the
  account's id and the config home on its command line. It reads the account's token from its
  keyring item and writes it to git's pipe alone: never to an argument, to the environment or
  to a file.
- **It is named only on the command line of the git the window runs**, and only while the
  window's thread says so (`as_the_window`, which takes the host's scope). It comes after
  `credential.helper=`, which drops every helper configured before it, whether per host or
  not. A call the app runs for a chat (a brokered call) never gets it, and the `purlis` command
  never names it. A host with no signed-in account keeps the CLI's helper.
- **It answers `get` only**: over HTTPS, for the named account's own host, from a signed-in
  account. `store`, `erase` and any other operation get nothing. Started in a chat's process
  tree, it answers nothing and says why on its standard error.
- **The window's save, fetch, push and clone use it**, the person's clone from Get a project
  (#1815) included. The clone and push it serves are tested against a local HTTP forge that
  demands credentials, with no forge CLI present.
