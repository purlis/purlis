### Fixed

- **A new project's steward no longer says it has no other personas to route to.** A project
  made from a stack template has that template's personas, but the steward `init` wrote still
  said there were none. Its routing section now points at them instead: `purlis persona list`
  names them and each lives in `personas/<name>/persona.md`. A steward already on disk is yours
  and is left as it is (#882).
