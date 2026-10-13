### Fixed

- **A git purlis stops is stopped with everything it started.** When a git purlis ran from the
  window passed its deadline or was called off (a rebase in a save or an update, a push, a
  fetch, a clone you cancelled), purlis ended git itself and reported the call as stopped, but
  the programs that git had started went on: a filter, a credential helper, and under a `pull`
  another git that could keep writing the working tree and holding its lock. Each git now
  leads its own process group and the group is what is ended, asked first so that every git in
  it removes its locks, and killed only if it has not gone in two seconds. The forge's own
  command line (`gh`, `glab`) is ended the same way at its deadline. The `purlis` command in a
  terminal is unchanged: the gits it runs stay with it, so that Ctrl-C still reaches them.
