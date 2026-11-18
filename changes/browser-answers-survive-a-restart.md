### Changed

- **A chat's browser Allow and a pool login opt-in survive a restart.** The answer you gave a
  chat's browser stays with that chat until it closes, on this machine only. A login you allowed
  on a pool's runner stays allowed on this machine until the pool, its image or its pin changes
  or you remove it (#7190).
- **A runner's browsers keep running when you quit purlis**, and are taken back when it reopens.
  A forgotten pool's browsers are stopped on its machine, or listed as left there (#7187).

### Fixed

- Starting browsers on two runners no longer waits for one window-wide lock (#7187).
