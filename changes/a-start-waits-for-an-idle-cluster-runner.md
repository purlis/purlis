### Changed

- **A chat started on an idle cluster runner waits for it.** The start used to be refused while the
  runner came back, and had to be started again. It now runs once the cluster operator vouches for
  the runner, for up to the pool's longest wait for that (#4403).
- **A cluster pool's retention holds at two hours at least, with an hour's notice at least.** A Pool
  that says less is read as those times, so a volume is not lost on a typed `30s`. Time spent stopped
  while an upstream object is in the way no longer counts toward a Claim's retention (#4405).
