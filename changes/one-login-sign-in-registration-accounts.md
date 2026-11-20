### Added

- **One login: pin the application an owner made on your GitHub Enterprise Server or self-hosted
  GitLab.** *Use this application* in Settings › You › Accounts keeps the application's client id on
  this machine, with the organization or group you expect to own it. Before you sign in, the window
  shows what to check on the forge's own page, and a self-hosted GitLab sign-in waits for your press
  before it opens the browser. It gives you a request to send an owner who has not made one. A client
  id a teammate sends has no effect until you press. Pinning another application raises a Notice for
  each account signed in through the old one.
- **Account for each project.** When you clone a repo from the window or press *Trust this clone*,
  purlis records its host, owner and name, and the forge's number for it, so a rename is the same repo
  and a lookalike is not. With a personal and an enterprise-managed account on one host, Settings ›
  You › Accounts lists each repo with the account you chose for it, and asks where you have not
  chosen. An account that signed out, or that was signed in through an application the host no longer
  pins, is shown as such and is never replaced by the other one.
- **Allow CI files.** On a GitHub account, a second sign-in asks GitHub for the permission to push CI
  files, and purlis keeps it apart from your everyday sign-in. Nothing uses it yet: pushes that change
  a CI file are held for your approval in a later release.

### Changed

- Signing in, pinning an application, choosing an account and trusting a clone are done only by your
  press in the window, never from a chat, the command line or a socket.

### Security

- If GitHub adds the CI-file permission to your everyday sign-in, purlis shows a Notice that says how
  to remove it, and it does not hand that sign-in to a chat's route while it holds the permission.
