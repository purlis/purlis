### Fixed

- **Moving a pool to a new runner leaves nothing behind.** If a move stops after Docker made the
  new runner and before purlis kept it, the next move removes that container and its home
  volume first, then makes the runner (#2869).
