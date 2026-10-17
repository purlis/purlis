### Added

- **A runner can be a container on this machine.** purlis's core makes a Docker pool's runner
  from the runner image, by its digest, confined the same way every time: Docker's default
  seccomp profile, every capability dropped, no new privileges, a user who is not root, its home
  on a volume of its own and no folder of this machine. It checks the container is still
  confined so before it pairs and before every link. The runner makes a link key of its own when
  it is paired, and the desktop pins it; `purlis serve` takes the link from that one desktop
  through `purlis bridge`, which the connector runs with `docker exec`. Adding a pool from the
  window comes next (#2044).
