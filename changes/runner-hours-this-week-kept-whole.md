### Fixed

- **Devices: a pool's hours this week stay whole when the record is full.** When
  `runner-hours.jsonl` is over its size, purlis now drops first the runs that ended before this
  week, which count nothing toward it, then the rest by when they ended, and a run still open
  last. Before, it dropped the runs that started first, so a long run could lose its hours this
  week (#3723).
