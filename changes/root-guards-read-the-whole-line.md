### Changed

- **The project-root guards read a git command whose words the shell fills in as unknown.** A
  git subcommand, or one of git's own options, built from a variable, a quoted substitution or a
  function's arguments is refused in the project root unless a branch move or reset is spelled
  among the words after it, which is then judged as written. An unquoted command substitution
  among git's own options is refused: quote it (`git -C "$(…)" …`) and purlis reads it as one
  word. A function's body is read wherever the shell may call it. Spell the command out and purlis reads
  it.

- **An earlier command on the line that may change the repository makes the guards judge what
  follows as unknown.** After it, an alias is not read from the configuration as it stood, a
  `git checkout <word>` is not taken to restore a file, and a `git reset` or `git update-ref` is
  not measured against the commits the branch held before the line ran. Run the command on its
  own and purlis reads and measures it as before.

- **Every `git reset` mode, and `git update-ref` of the root's branch, is measured for unpushed
  commits.** `--soft` and `--mixed` keep the files but take the commits off the branch, so a reset
  in any mode, or a ref update of `HEAD` or the branch the project root is on, that would leave
  commits no upstream has with only the reflog is refused until `purlis save` pushes them. A
  commit only the shell can name is measured as the furthest back, and `git update-ref --stdin`
  in the root is refused.

- **`git symbolic-ref HEAD <ref>` in the project root is refused as a branch move.** It points
  the root at another branch without touching a file. Reading a symbolic ref stays allowed.
