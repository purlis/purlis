### Changed

- **Open in terminal on a pool's profile now types `purlis profile sign-in <name> --pool <pool>
  --in-terminal`.** It checks the pool, as the window does, every time it runs, including when you
  run it again from your shell's history. Then it runs the runner's own sign-in in that tab for
  you to drive. The line the window just typed goes straight on. Run again later, it asks you in
  the purlis window first.

### Security

- **A cluster runner's sign-in goes to the runner purlis paired with.** The window now signs a
  profile in on a cluster runner over its own link to it, which proves the runner by its key. A
  runner on an older purlis, and a sign-in from the command line, still go through `kubectl
  exec`. Those now read the pod's uid right before the sign-in starts and again once the sign-in
  has reached it, before it shows you anything or passes anything on. If a new pod took the
  runner's name in between, the sign-in stops and none of it counts.
