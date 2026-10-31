### Fixed

- **A Docker pool keeps its sign-ins when the app updates.** The pool's new runner takes the old
  runner's sign-ins once the old runner's last chat has ended, carried between the two runners'
  volumes on the Docker host just before the old one is removed, so a profile signed in on the
  pool stays signed in and no sign-in is held by two runners at once. Until then, a new chat on
  the new runner asks to sign in there. Only the sign-in files are carried, nothing passes
  through your machine, and a sign-in that cannot be carried never holds the update back: that
  profile is signed in again (#4011, #4055).
