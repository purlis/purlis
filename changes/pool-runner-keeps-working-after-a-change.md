### Fixed

- **A pool keeps working after its runner is made again or updated**. The first chat after
  **Make the runner again** was refused with "no persona on this plane", since the new runner was
  never sent the project. And once a runner updated beside an old one had taken over its sign-ins,
  every chat on that pool was refused, saying its profile "names a folder outside purlis's profiles
  folder". Now the new runner is sent the project, and carrying sign-ins leaves a running runner's
  folders where they are.
