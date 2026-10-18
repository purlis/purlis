### Fixed

- **A tool declared under another name runs on macOS.** A vault tool whose declared name differs
  from its program's file name (`gzcat` for `gunzip`) failed to start through the app's sandbox on
  macOS with "Operation not permitted". purlis now starts the tool through a link the
  sandbox lets it read (#2617).
