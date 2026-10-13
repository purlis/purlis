### Security

- **The guards read a little more of what a shell runs.** A heredoc fed to any shell the guards
  read a string for counts as commands, never as data. The project-root guard reads an `eval`
  under the shell's `set -a`. Each change only adds refusals.

### Fixed

- **A `commit -v` under a comment mark of several characters commits again.** Where git's
  `core.commentString` (or `core.commentChar`) is more than one character, the commit-message
  scan now sees git's scissors line, so a verbose commit that removes a credential is no
  longer refused for the diff git puts below it.
