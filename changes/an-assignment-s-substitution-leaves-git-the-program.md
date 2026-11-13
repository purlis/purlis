### Fixed

- **A substitution in an assignment before git no longer blocks the commit.** The branch-folder
  guard refused `FOO=$(date) git commit -m x`, in any permission mode, saying it could not tell
  what the git command does. An assignment's value is one word whatever it prints, so git is the
  program, read as it is in `FOO="$(date)" git commit -m x`. That holds after several
  assignments and after `env` too. When the substitution itself may hold the git that runs,
  as in `FOO=$(git status) git checkout feature`, a branch move after it is now refused where
  it was allowed.
