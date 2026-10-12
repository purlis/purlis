### Changed

- **The opencode plugin list names the last places opencode loads plugins from.** The settings
  tab's harness-plugin list now also names what opencode loads for a chat in one of a
  workspace's branch folders (each its own git top), what `~/.opencode/` holds (its two configs
  and its `plugin/` and `plugins/` scripts; never a `~/opencode.json`), and, for a project that is
  not a git repository of its own, each folder above it up to the repository it sits in, or up to
  `/`, named by its path. A walk ends only at a `.git` git itself takes as a repository, as
  opencode's does, and each folder is read through the one handle purlis opened on it (#1590,
  #1828).
