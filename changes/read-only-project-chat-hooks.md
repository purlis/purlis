### Fixed

- **A chat writes nothing committed to a project this purlis may only read.** In a project
  that requires a feature this purlis lacks, or declares a newer format, a chat's hooks no
  longer log its skill uses into `personas/_skills/`. A session record, memory (a workspace's
  or a persona's), a todo, a workspace's manifest, vision or sections, a new workspace, and a
  handoff's row in the dispatch log are each refused, saying nothing was written and why,
  whichever way the write was asked for. The project is not saved (committed or pushed), by
  auto-save after a chat ends or otherwise, and no extension's program is started at a chat's
  start or told of an event there. Every guard still refuses what it refused before. The window
  says so in the project's tab before a release carries this rule (#826).
