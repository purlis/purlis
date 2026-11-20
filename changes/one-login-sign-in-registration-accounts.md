### Added

- **One login: pin the application an owner made on your GitHub Enterprise Server or self-hosted
  GitLab, and sign in once per host.** *Use this application* in Settings › You › Accounts keeps the
  application's client id on this machine, with the organization or group you expect to own it. The
  window tells you what to check on the forge's own page before you sign in, and gives you a request
  to send an owner who has not made one. A client id a teammate sends has no effect until you press.
- **Choose which of your accounts serves a project.** With a personal and an enterprise-managed
  account on one host, each project uses the one you chose, and no other: if that account signs out,
  the project says so instead of switching to the other. One account on a host asks nothing.
- **A repo is recorded as yours when you clone it from the window or press *Trust this clone*:** its
  host, owner and name, and the forge's number for it, so a rename is the same repo and a lookalike is
  not.
- **CI-file pushes use a second permission you give once, the first time one is needed.** The
  everyday sign-in never has GitHub's `workflow` scope; the second is kept separately, only the app
  uses it for a push you approved, and no chat's route carries it. If GitHub merges the two, purlis
  does not offer the push and says to make it from your own terminal.

### Changed

- Signing in, pinning an application, choosing an account and trusting a clone are done only by your
  press in the window, never from a chat, the command line or a socket.
