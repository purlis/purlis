### Added

- **Every other end of a sign-in on a pool's runner** (#4043). Where purlis can't follow a
  login, the strip shows the runner's live terminal in place, and purlis checks the result when
  it ends. A sign-in that fails says the vendor's reason and offers Open the page again. Each
  profile's line says where it stands: not signed in, signing in…, signed in, sign-in failed or
  needs you in the terminal. Sign out on a signed-in line asks first, then runs the harness's own
  sign-out on the runner.

### Fixed

- **Cancel ends a sign-in on the runner too** (#4043). A cancelled, retried or timed-out sign-in
  used to run on inside the container until it stopped by itself. It now ends there at once.
