### Added

- **Settings lists a workspace's own opencode plugins.** At the Workspace level, the opencode
  group also lists the plugins and configs in the workspace's own `.opencode/`, in each folder
  between the workspace and the git top, and in each of its clones, each named by its place in
  the project. purlis never reads through a link there: a linked `.opencode` or plugin folder is
  named, not followed (#1590).
