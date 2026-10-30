### Fixed

- **A task that reports done while its own monitors or background shells still run is no longer
  ended under them.** Its program is ended once that work has woken it and its turn ends with
  nothing in flight, and never later than the two minutes a reporting turn is given (#3921).
