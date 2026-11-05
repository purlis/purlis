### Changed

- **A clone in a workspace's `repos/` folder is seen wherever a clone at its root is.** The repo
  list, Explorer's repo rows, the Panel's Repos view, `purlis doctor`'s clone checks, the
  workspace layer's exclude block, the sandbox's protection of each clone's git folder, a chat's
  repo skills and the code sent to runners all find it there. Where a workspace holds a repo both
  in `repos/` and at its root under one name, the one in `repos/` is the clone (ADR 0114).
