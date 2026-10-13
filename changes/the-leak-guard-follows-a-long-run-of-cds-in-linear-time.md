### Fixed

- **A long line of `cd`s no longer makes the leak guard slow.** The guard follows every `cd`
  on a line, since what a later command reads is read from where the shell then stands, and
  it spelt the whole way there again for each one: a 64 KB line of them took it 2.8 seconds
  in a test build, the square of its length. It now carries where the shell stands from one
  `cd` to the next. What it answers is unchanged.
