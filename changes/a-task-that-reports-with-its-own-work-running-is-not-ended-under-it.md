### Fixed

- **A task that reports done while its own monitors or background shells still run is no longer
  ended under them.** Its program is ended once that work has ended and its turn ends with
  nothing in flight. The two minutes a reporting turn is given run from when that work ends, and
  a cancel still stops the task and its work at once (#3921).
