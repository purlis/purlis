### Fixed

- **Removing a pool whose runner is on another Docker engine says what stays there.** It asks in those
  words, touches nothing on the engine Docker reaches now, and names the container and volume to remove
  once Docker points back. A pool whose engine is gone for good can still be removed (#4325).
