### Fixed

- **A fresh runner's check of a very large image has a time limit, and says when it runs out.** A check looks through the runner's files for at most 5 minutes. Past that the runner stays, and the window says why and what helps. Discard now counts even while a check is running: the runner goes when that check ends. It used to fail after the pool's longest wait for an answer, 30 seconds by default, while the look ran on and the next try started another beside it (#5312).
