### Fixed

- **A file named `gh`, `glab` or `git` can be committed from a chat.** The merge guard no longer
  takes a path that `purlis worktree commit` is given (`tests/fakes/glab`) for the program of that
  name, so the commit is no longer refused. A real `gh` or `glab` call on the same line, or in a
  substitution in the commit message, is still held. A refusal earned only by the text of a
  heredoc that a program such as `python3 -` runs now says to write the file with the editor tool
  instead.
