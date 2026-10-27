### Fixed

- **When a pool's runner cannot start a browser safely, the notice says which runner.** The
  notice about the browser engine's sandbox now names the pool whose runner refused, and its
  command is the one to run on that runner, not on your own machine. Your own machine's notice
  is unchanged and can stand beside it, and the chat is told a browser is not available on that
  runner (#2990).
