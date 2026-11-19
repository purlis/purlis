### Changed

- **A chat's browser Allow and a pool login opt-in survive a restart.** The answer you gave a
  chat's browser stays with that chat, closed or not, until purlis lets go of its record, on this machine only. A login you allowed
  on a pool's runner stays allowed on this machine until the pool, its image or its pin changes
  or you remove it, and is listed in the Granted list by its name with Remove (#7190). A removal purlis cannot make is reported, and the login no longer applies. A kept browser
  answer is checked against the chat's current reach: where the reach narrowed meanwhile, you are
  asked again.
- **A runner's browsers keep running when you quit purlis**, and are taken back when it reopens.
  A forgotten pool's browsers are stopped on its machine, or listed as left there (#7187).

### Fixed

- Starting browsers on two runners no longer waits for one window-wide lock (#7187).
