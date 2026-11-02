### Fixed

- **An older runner that stops before its chats end keeps its home.** After an update or a Replace,
  a runner that stopped while chats may have been running on it was removed with everything they
  left. purlis now keeps it until you remove it with Remove runner in Devices, and the pool's row
  in Settings says so. A purlis from before this refuses a `runners.json` holding `last_stop` or
  `held`, as it refuses any key it does not know, so a person who goes back to one removes those
  keys first.
