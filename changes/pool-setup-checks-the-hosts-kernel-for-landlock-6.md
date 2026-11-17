### Added

- **Pool setup says when a pool's Docker host has a kernel too old for purlis's sandbox, before
  you start a chat there.** A container runs on its host's kernel, and every chat on a runner
  needs Landlock version 6 (Linux 6.12 or later). Connect, Check again and `purlis pool doctor`
  now name the host's kernel and how to get to 6.12. On Ubuntu 24.04 that is
  `sudo apt install --install-recommends linux-generic-hwe-24.04` and a restart. Start a chat no
  longer offers that pool on the Runs on row, and says why.
