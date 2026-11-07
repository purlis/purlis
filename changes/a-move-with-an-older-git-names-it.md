### Fixed

- **Moving a chat to a pool with git older than 2.40 says which git and what to do.** The move
  used to stop on git's own usage text ("unknown option: --attr-source=…"). Now it names the
  git that ran and its version, says a move needs git 2.40 or later, and leaves the chat where
  it was. This applies on your machine and on the runner.
