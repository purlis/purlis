### Fixed

- **The save status is right on a very big project on a slow disk.** On a project where
  reading every file takes git more than 30 s, purlis stopped each status at 30 s. The status
  bar then showed nothing to save, even when there were changes. The one status that prepares
  git's caches now gets two minutes, so it finishes once and later reads are quick (#5486).
