### Fixed

- **A runner with only its home left is just removed.** After a removal that took its container but
  not its home volume, a draining runner no longer waits for an answer it can't give.
- **A sign-in Docker failed to carry gets one more try.** If the removal failed too, the next look
  carries it, so the profile isn't signed in again.
- **A pool's sign-out runs once on each draining runner.**
