### Fixed

- **A chat's guard no longer slows down with every chat you have open.** Before each tool call
  the guard asks the app for its chat's policy, and each answer read the project's committed
  settings from git, twice, so a dozen chats meant a dozen reads at once. Chats that ask together
  now share one read that starts after each of them asked, so a settings change you committed
  is still seen by the next call.
