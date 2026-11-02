### Fixed

- **A command the doctor suggests pastes as the command it names, whatever the path.** The
  `plane root` hint's `git -C … checkout …` and the `index lock` hint's `rm -f …` quote the path
  (and the branch) for the shell, and never shorten it: a plane under a folder with a space, a
  quote or a `$` in its name no longer pastes as a broken or different command (#4830).
