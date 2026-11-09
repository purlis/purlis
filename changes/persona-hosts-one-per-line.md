### Changed

- **A persona's own sandbox hosts are written one per line, sorted.** The Settings form writes
  `[sandbox.personas.<name>] hosts` as the project's hosts are written, one host per line in
  order and once each, so two machines that each add a persona host merge without a merge
  commit (#5853).
