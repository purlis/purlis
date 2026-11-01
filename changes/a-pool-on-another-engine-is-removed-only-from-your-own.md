### Changed

- Removing a pool whose runner is on another Docker engine now needs Docker to be using an engine
  shown to be yours. Otherwise nothing is removed, and the message says what to do.
