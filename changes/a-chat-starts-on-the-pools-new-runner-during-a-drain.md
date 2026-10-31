### Fixed

- **Starting a chat while a pool's runner is being replaced just works.** The chat starts on the
  pool's new runner, and chats still on the old runner stay reachable until they end. It used to
  be refused with "this runner is being replaced" until purlis restarted.
- **A slow machine or runner no longer fails a chat's start after 30 seconds.** Start keeps saying
  "Starting…" for up to 3 minutes. Past that it says what to do, and a chat that comes up after
  purlis stopped waiting is closed.
