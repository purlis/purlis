### Fixed

- **A browser call answers at once while another browser is stuck.** When purlis could not end a
  chat's browser yet, it now keeps trying on a timer of its own, waiting longer between tries,
  so no chat's browser call waits on it. A browser that is not ended yet still counts toward the
  project's three until it is (#2283).
