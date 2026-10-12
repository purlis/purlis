### Added

- **Sign in to GitHub or GitLab with a token, on any host.** Settings › You › Accounts lists
  this machine's forge accounts, with Sign out, and **Sign in with a token…** takes github.com,
  gitlab.com, a GitHub Enterprise server or a self-managed GitLab. purlis lists the exact scopes
  and opens the forge's page that makes the token, checks the token with the forge, refuses one
  missing a scope by naming it, and keeps it in your system keychain as an item of its own,
  never in a vault and never where a chat can reach it. Sign out deletes it from the keychain.
  The welcome's **Sign in to GitHub or GitLab** opens the same sign-in, and the forge work you
  do in the window uses the account (#1814, #1816).
