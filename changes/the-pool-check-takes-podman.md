### Added

- **A Docker pool runs on Podman.** The pool's container check takes what rootful and rootless
  Podman write for a correctly confined container, as measured on CI, and still refuses a
  loosened one (#2680).
