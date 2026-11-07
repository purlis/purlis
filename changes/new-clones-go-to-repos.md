### Changed

- **New clones go to a workspace's `repos/` folder.** Every clone, from `purlis clone`, the
  window, `purlis init --clone-this-repo` or opening a repo as a new project, lands in
  `workspaces/<ws>/repos/<name>`, apart from the workspace's own notes and files. Clones already at
  a workspace's root stay where they are and keep working beside the new ones. `repos` is now a
  reserved name: a clone named `repos` is refused, and so is a new clone while an old clone named
  `repos` stands in the way, with how to rename it. The workspace layout is version 6, which
  `purlis workspace reinit` stamps without creating or moving anything, and a chat is told to keep
  its notes, scripts and artifacts out of `repos/` (ADR 0114, #5373).
