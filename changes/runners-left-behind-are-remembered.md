### Added

- **A runner left on another Docker engine is remembered.** When you remove a pool while Docker
  points at another engine than the one that made its runner, purlis still forgets the pool, and
  now keeps the runner's container and home volume in a record of their own.
  `purlis pool left-behind` lists them with the commands that remove them, and
  `purlis pool left-behind --remove` removes them once Docker points back at that engine.
