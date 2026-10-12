### Added

- **Git's network calls are in the network log.** A fetch, push or remote listing purlis starts
  is listed with its host, the git operation and whether git finished it. The repo's path, a
  ref and a credential are never listed (#1022).
