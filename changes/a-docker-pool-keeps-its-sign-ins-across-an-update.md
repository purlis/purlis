### Fixed

- **A Docker pool keeps its sign-ins when the app updates.** The pool's new runner takes the
  old runner's profiles, copied between the two runners' volumes on the Docker host before the
  new runner is paired, so a profile signed in on the pool stays signed in. Nothing passes
  through your machine, and a copy that fails leaves the pool on its old runner until the next
  try (#4011).
