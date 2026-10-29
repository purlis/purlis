### Added

- **Devices: the disk each pool's runners hold.** The use table under the devices has a Disk held
  column: the size of each pool's runners' home volumes together, a draining runner's included,
  as `docker system df` (or Podman's) reports it. Reading it starts no suspended runner, and a
  slow count holds up nothing else in the view. A home Docker does not list is said apart, never
  counted as nothing (#3493).
