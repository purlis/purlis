### Fixed

- **A command purlis suggests for a git repository pastes as the command it names, whatever the
  path.** The doctor's `plane root` hint (`git -C … checkout …`), its `index lock` hint and the
  stale-lock remedy a save or update prints (`rm -f …`) quote the path (and the branch) for the
  shell and never shorten it: a folder with a space, a quote or a `$` in its name no longer
  pastes as a broken or different command (#4830).
