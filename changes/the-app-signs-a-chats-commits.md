### Added

- **The commits a chat makes are signed, so a repository that requires signed commits takes
  them.** `purlis signing setup` makes a key (or takes yours), keeps it in this machine's
  keychain, adds it to your GitHub account as a signing key and checks that GitHub has it, in
  three plain steps. From then on the app signs each commit it makes for a sandboxed chat, and
  the commits `purlis worktree update` makes by a rebase or a merge, and GitHub shows them as
  Verified. The chat never sees the key. With no key, the commit is still made, and the chat is
  told it is unsigned and how to fix that. `purlis signing check <owner/repo> <commit>` asks
  GitHub how a pushed commit shows (#5480).
