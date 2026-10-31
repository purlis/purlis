### Changed

- **A pool keeps only the profiles chosen for it.** A Docker pool's runner no longer keeps every
  one of this machine's profiles: it keeps the profile a new chat in the open project gets, or
  the ones chosen for the pool, kept in `runners.json` as the pool's `profiles`. Each pool's row
  in Settings › This machine › Pools has **Add a profile…**, and **Remove** on each profile,
  which asks first: the runner forgets that profile, its sign-in there with it. A chat that
  starts on or moves to the pool on another profile adds it (#4047).
