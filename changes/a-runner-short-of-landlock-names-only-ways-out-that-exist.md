### Fixed

- **A chat refused on a pool whose kernel lacks Landlock 6 now names only ways out that work.**
  It no longer says to start the chat on a Claude Code profile, which runs in the same sandbox on
  Linux, or without the sandbox, which a runner refuses. It says to update the pool's Docker host
  to Linux 6.12 or later, since a container runs on its host's kernel, or to pick This machine on
  the Runs on row. Start a chat no longer offers "Start without the sandbox" for a chat on a pool.
  On your own Linux machine the refusal no longer names a Claude Code profile either.
