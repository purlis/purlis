### Added

- **A runner's key is kept once, and removing a runner removes it whole.** purlis keeps each
  Docker pool runner's device id and key in `runners.json` the first time it is paired, and
  refuses a later answer with another key. Removing a runner unpairs it while its link is up,
  then removes its container and home volume. Anything left is named with the `docker`
  command that removes it, and the runner stays listed until it is gone (#2525).
