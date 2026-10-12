### Added

- **Use your existing gh or glab login.** Where `gh` or `glab` is signed in to github.com or
  gitlab.com and purlis has no account there yet, the welcome's **Sign in to GitHub or GitLab**
  and Settings › You › Accounts offer **Use my gh login** (or glab). The offer says first what it
  does: the token the tool keeps is copied into purlis's own item in your system keychain, which
  purlis never hands to a chat. A chat that runs the tool itself can still use its own stored
  login, so sign out of the tool if you don't want that. **Not now** comes first,
  and nothing is imported unless you press **Use my … login**. The token is checked with the
  forge like a pasted one, never shown in the window, and never reaches a chat. The tool keeps
  its own login (#1376).
