### Fixed

- **A sandboxed chat's builds use the project's compile cache from the first one.** With
  `compile-cache = "sccache"`, a Claude Code chat's first `cargo build` failed with "Operation not
  permitted", because sccache's own start binds a socket the chat's sandbox refuses. The chat's
  commands now start its own sccache server on the one socket the sandbox allows, and start it
  again when it has stopped and left its socket behind. The file that hands a chat the compile
  cache is now written even when the chat has no ssh route (#7935).
