### Fixed

- **Starting a chat while a pool's runner is being replaced just works.** The chat starts on the
  pool's new runner, and chats still on the old runner stay reachable until they end. It used to
  be refused with "this runner is being replaced" until purlis restarted.
- **A slow machine or runner no longer fails a chat's start.** The dialog says "Starting the session
  host…" for as long as it takes, Cancel works throughout, and it shows an error only when the host
  has actually failed. A chat that comes up after you cancel is closed.
