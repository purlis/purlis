### Fixed

- **Ending a chat no longer freezes the window when its session host is slow to answer.** The
  window waited up to 30 seconds for the host to confirm the close, and nothing in it could be
  pressed or typed in meanwhile. It now waits at most a second, and the host's late answer is
  still logged (#6227).
