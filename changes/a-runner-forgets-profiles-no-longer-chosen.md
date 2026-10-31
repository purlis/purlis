### Changed

- **A runner keeps only the profiles its pool names.** As a pool's profiles are shown, and as
  the first chat of a run starts there, a runner of this version names the profiles it keeps
  for this machine. It forgets, sign-in included, each one no longer chosen for the pool or no
  longer on this machine. A pool made before pools kept only their chosen profiles now names
  every profile its runner kept, so each shows on its row to remove (#4060). Removing a profile
  in Settings › You › Profiles removes it from each pool's runner that kept it; a runner that
  is away is asked again at its next chat or when its pool is shown, and one of a purlis before
  1.52 keeps it, which the removal says (#3849). Removing a profile from a pool whose runner has
  one of the same name of its own now takes it off the pool's record.
