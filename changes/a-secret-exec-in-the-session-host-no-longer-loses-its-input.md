### Fixed

- **A `secret exec` run from a chat in the session host no longer waits forever now and then.**
  When the command's input arrived, or ended, before the window had started the command, the
  window lost it, and the command waited for input that never came. Piped input, which ends at
  once, hit this most. The window now keeps that input for the command (#2271).
