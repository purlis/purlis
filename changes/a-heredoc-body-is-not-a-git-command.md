### Fixed

- **A heredoc that names git is not a git command.** A chat that wrote a file with a heredoc,
  such as an entry in a ledger that mentions `git push` or a script saved to a scratch file, was
  refused as though it ran the git its text named, by the branch-folder guard, the project-git
  guard, the plane-root guard, the commit-hook guard and the release floor. A body that no shell
  or other program that runs text is handed is now read as data. Where its delimiter is unquoted
  (`<<EOF`), the command substitutions in it are still read, because the shell runs them, and a
  command after the body is judged as before (#6293).
