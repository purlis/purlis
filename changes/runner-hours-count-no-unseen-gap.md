### Fixed

- **Devices: hours this week no longer count a gap purlis could not see.** When a runner's
  container was stopped and started again outside purlis between two reads, Docker says only
  the newer run's times, and purlis counted everything from the older run's start to the newer
  one's as running. Where Docker does not say when the older run ended, purlis now counts none
  of that gap, so a pool's hours and money are not raised by a time it never saw (#3720).
