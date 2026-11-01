### Changed

- **A profile over its pool's limit now says so and offers to raise it.** A pool has limits on what a
  profile may send to its runner, by size (128 MiB by default) and by files (8,192 by default). Going
  over used to leave part of the profile behind, with a note on the chat's tab. Now nothing is sent:
  Start a chat says how much is needed and suggests a value you can edit, and applying it saves the
  value to the pool and starts the chat. A value you never set follows purlis's default (#4279).
