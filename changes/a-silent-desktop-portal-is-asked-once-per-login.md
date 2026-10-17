### Changed

- **On Linux, a desktop portal that did not answer is asked once per login session, not at
  every launch.** On a machine whose session bus has a portal that cannot start (a CI runner, a
  container, a bare window manager), only the first launch after login waits the portal's 300 ms
  before it starts without the bus. Later launches in the same session start straight away. The
  portal is asked again at the next login, or at the next launch once it has answered (ADR 0026,
  #2432).
