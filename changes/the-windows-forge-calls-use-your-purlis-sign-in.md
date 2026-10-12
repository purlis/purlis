### Changed

- **The window's forge calls use your purlis sign-in, so `gh` and `glab` are optional.** Once
  you are signed in to a host in Settings › You › Accounts, the workspace repo picker, the
  Doctor's discover, the save button and the workspace's LIVE switch (and the pull or merge
  request they open), and the workspace panel's request and CI column ask that forge over HTTPS
  with your sign-in, read from the keychain for each request. A host with no purlis account
  still uses `gh` or `glab` when they are installed; when they are not, the message says to
  sign in under Settings › You › Accounts. Chats, `purlis` commands and saves nobody pressed for
  never use the sign-in (#1816).
