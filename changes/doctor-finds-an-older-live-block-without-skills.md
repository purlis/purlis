### Added

- **The doctor finds a LIVE workspace whose skills are not shared yet.** A project set up
  before LIVE workspaces shared their skills keeps them on this machine. The new `live skills`
  row names each such workspace and the lines it lacks, and any level whose local skills git
  does not ignore, a persona's included. `purlis doctor --fix live-skills`, the Doctor dialog's
  Fix button, or bare `purlis doctor --fix` adds exactly those lines and nothing else (#2875).
