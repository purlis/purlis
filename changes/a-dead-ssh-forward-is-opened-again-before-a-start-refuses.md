### Fixed

- **A start on a pool over SSH opens its forward again once before it refuses.** After the
  pool's machine restarted, the forward could look alive while nothing crossed it, and the first
  start said Docker didn't answer through SSH until you tried again.
