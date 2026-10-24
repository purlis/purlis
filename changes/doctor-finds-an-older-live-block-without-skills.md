### Added

- **The doctor finds skills an older project does not publish or keep local.** A project set up
  before LIVE workspaces published their skills keeps them on this machine, and one set up
  before local skills were kept out of git can commit them. The new `live skills` row names
  each LIVE workspace whose skills are not published and the local skills of a project,
  workspace or persona that git does not ignore. `purlis doctor --fix live-skills`, the Doctor
  dialog's Fix button, or bare `purlis doctor --fix` adds exactly the missing lines and nothing
  else (#2875).
