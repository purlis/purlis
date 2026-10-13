### Added

- **Sign in with GitLab, in your browser.** **Sign in with GitLab**, on the welcome's sign-in
  page and in Settings › You › Accounts, opens gitlab.com in your own browser. You approve
  purlis there, the browser comes back to purlis by itself, and the account is listed with its
  sign-in in your system keychain, never where a chat can reach it. No token to make or paste.
  purlis asks for the `api` scope only. Cancel stops it at once, and a sign-in that cannot
  finish says why and what to do. Signing out ends the token on gitlab.com too, and says so if
  it could not. A self-managed GitLab still signs in with a token (#730).
