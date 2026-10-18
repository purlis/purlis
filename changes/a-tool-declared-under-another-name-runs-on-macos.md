### Fixed

- **A tool declared under another name runs on macOS.** A vault tool whose declared name differs
  from its program's file name (`gzcat` for `gunzip`) failed to start through the app's sandbox on
  macOS with "Operation not permitted". The run's sandbox now gives back the link it starts the
  tool by under that link's own name, and nothing it points at (#2617).
