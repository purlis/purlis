### Changed

- **A github.com clone made with `purlis clone` says a chat's gh gets no Agent identity token for
  it.** Only the window records a clone for the GitHub App mint, so `purlis clone` now says so,
  says a chat already running will not pick one up until it restarts, and names the way out
  (re-clone from the window, or press *Trust this clone* there). A chat's own clone says it in the
  command's output. A chat starting in a workspace with such a clone is told at its start, and a
  `gh` that fails inside one is told which clone it was in. Nothing is refused: a `gh` run there
  for a repo the window recorded works as before. Not covered: a clone made after the chat
  started, an origin with several URLs or none, and a clone whose `.git` a chat can write (#7196).
