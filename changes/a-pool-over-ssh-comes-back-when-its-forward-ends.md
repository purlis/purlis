### Fixed

- **A pool over SSH comes back by itself when its connection ends.** If the window's SSH
  connection to a pool's machine ended while the window wasn't using that pool, chats there could
  fail for hours with "Cannot connect to the Docker daemon", until you restarted purlis. purlis now
  opens the connection again the next time anything uses the pool.
