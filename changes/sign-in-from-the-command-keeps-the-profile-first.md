### Changed

- **`purlis profile sign-in <profile> --pool <pool>` puts the profile on the runner first.** While the
  purlis window is running, a profile the runner does not keep yet goes there as Sign in on its line in
  Settings › Pools does, and the pool names it from then on. With no window running, the command says so
  and starts nothing (#4112).
