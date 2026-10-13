### Added

- **`purlis doctor` warns when your git reads any bare repository.** The new `git bare repos`
  row warns while no git config you use sets `safe.bareRepository = explicit`, and says why: a
  sandboxed chat can write a folder git takes for a bare repository, and a git run inside it
  later can run a program its config names (#1100).
