### Added

- **`purlis profile sign-in <profile> --pool <pool>` on a cluster pool.** The command now signs a
  profile in on the runner of a cluster pool you marked *This is my own*, while the cluster vouches
  for its claim, the same way the row in Settings › Pools does. An unmarked pool, or one not
  vouched for at that moment, is refused in words and nothing runs there (#4164).
- **A cluster runner forgets what is no longer chosen.** Showing a marked cluster pool's profiles,
  or a chat's start there, has its runner forget the profiles no longer chosen for it, and removing
  a profile on this Mac has it forget that one too (#4183).

### Fixed

- **`purlis profile sign-in --pool` on a Docker pool** no longer stops at asking whether the runner
  keeps the profile: the question it asked the runner was one the runner refused.
