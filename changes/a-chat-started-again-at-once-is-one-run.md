### Fixed

- **A chat started again at once counts and records one run** (#2250). When a sandboxed Claude
  Code chat is started once more at its start because its project's settings changed, this
  machine's sandbox count and the event log say one run: the one that goes on.
