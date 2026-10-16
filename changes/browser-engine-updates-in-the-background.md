### Added

- **The browser engine stays current between purlis releases** (#2073). Once you have used a
  browser on this machine, the session host checks each day's signed manifest of Chrome for
  Testing in the background and takes a newer engine only when its Sigstore signature verifies
  as purlis's own manifest workflow on `main`. An engine still current is no longer refused for
  its age. Each check is listed in the network log as an updater read. With no update ever
  received, the engine your build pins keeps working.
