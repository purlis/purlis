### Fixed

- **A clone a runner's chat made comes back without a second copy or a stuck move.** Two clones
  started at once in a workspace with no `repos/` no longer fail because one took the folder away
  as the other used it. A clone the runner's chat made is no longer copied into `repos/` beside
  an old clone of the same name at the workspace's top, whether the names differ only in case or
  the old clone's `.git` is a link. An old clone named `repos` no longer stops a chat's work from
  coming back: only the runner's new clones stay on the runner, and the chat's tab says to rename
  that old clone so they can come back.
