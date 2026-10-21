### Added

- **A Docker pool's runner follows purlis's version after an update.** Each release builds
  purlis's runner image with its own version and names it in its update manifest, signed with
  the updater's key. After an update, a pool of purlis's own image gets a runner of the new
  version beside its old one: new chats start on it, and the old runner keeps its chats until
  they end, then is removed. A pool of your own image is never moved, and its row in the Pools
  group says its runner is behind (#2749).
