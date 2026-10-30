### Changed

- **A runner forgets the profiles no longer chosen for its pool.** As a pool's profiles are
  shown, and as the first chat of a run starts there, its runner names the profiles it keeps for
  this machine and forgets, sign-in included, each one no longer chosen for the pool, so a
  runner made before pools kept only their chosen profiles keeps just those now (#4060).
  Removing a profile in Settings › You › Profiles removes it from each pool's runner that kept
  it; a runner that is away forgets it when it next links, and one of a purlis before 1.52 keeps
  it, which the removal says (#3849). Removing a profile from a pool whose runner has one of the
  same name of its own now takes it off the pool's record.
