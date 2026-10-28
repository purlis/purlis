### Fixed

- **Devices: hours this week never count a gap purlis could not see.** When a runner's
  container was stopped and started again outside purlis between two reads, Docker says only
  the newer run's times, and purlis counted everything from the older run's start to the newer
  one's as running. It now counts nothing it did not see run, so a pool's hours and money are
  never more than what ran (#3720).
