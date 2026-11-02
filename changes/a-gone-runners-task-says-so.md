### Fixed

- **A task that ended with a gone runner says why.** Its report names the runner, and its finished row
  says it can't be reopened because the conversation went with the runner. A chat lost earlier, when the
  session host stopped, is told too once its runner goes, and a task in that state now reports instead of
  waiting for ever (#4420, #4421).
