### Fixed

- **A merge no longer fails as "no answer" when the machine is busy.** The command line's merge
  check waited 700 ms for the session host to decide, and a loaded machine could take longer, so
  `gh pr merge` was refused with no answer. It now waits up to 1.25 s for each read, and the Bash
  hook, the only one that merges, has five seconds in all (it had two). The Read, Edit and
  dispatch hooks keep their two seconds, and a merge not decided in time is still refused.
