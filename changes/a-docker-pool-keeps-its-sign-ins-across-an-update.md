### Fixed

- **A Docker pool keeps its sign-ins when the app updates.** The pool's new runner takes the old
  runner's sign-ins, carried between the two runners' volumes on the Docker host before the new
  runner is paired, so a profile signed in on the pool stays signed in. Only the sign-in files
  are carried, nothing passes through your machine, and a sign-in that cannot be carried never
  holds the update back: that profile is signed in again (#4011).
