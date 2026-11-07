### Added

- **Signed commits, set up from the window.** A **Signed commits** card in Settings › You ›
  Accounts sets up the key purlis signs your chats' commits with, in one dialog of three steps:
  - **Make your key.** purlis tells you before macOS asks for the Keychain.
  - **Add it to GitHub.** purlis adds it through your account, asking GitHub's permission once,
    or you add it yourself and the window carries on once GitHub has it.
  - **Check.** GitHub itself confirms a signed test as Verified.

  The card then shows "Verified on GitHub ✓" with the email your commits use. The key stays in
  this Mac's Keychain, and no chat can reach it (#5628).
