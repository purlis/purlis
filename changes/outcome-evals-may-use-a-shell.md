### Changed

- **A skill that runs its own scripts can pass its outcome evals.** Where a skill's
  `allowed-tools` names Bash, each eval's chat in a grade has a shell, in every arm. The shell
  writes only in the chat's own folder and reaches no network. The plan says which evals use one
  before anything runs, and a grade's token and dollar limits still hold (#4476).
