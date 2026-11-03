### Added

- **The app's log says where each chat's start at a launch spent its time.** Every chat put back
  at a launch logs one line with the time it waited at the restore gate, then the core's read,
  the sandbox's proxy, its composed line (the package caches with it) and the host's answer, and
  one more line when its harness first reports. A slow launch can now be read from
  `~/Library/Logs/dev.purlis.app/` (#4988).
