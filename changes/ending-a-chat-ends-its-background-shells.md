### Fixed

- **Ending a chat ends the background shells it started.** Claude Code runs each shell in a
  session of its own, so a background shell outlived its chat when purlis quit, restarted it,
  stopped it or settled a crashed session host, and ran on unseen. purlis now ends what a chat's
  harness started outside its process group along with it (#5749).
