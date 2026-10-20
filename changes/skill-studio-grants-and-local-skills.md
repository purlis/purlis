### Added

- **Skills reach the workspaces you grant them to, in the Skill studio preview.**
  - A project skill reaches a workspace's chats once you grant it: `purlis skill grant <name> all`, or name the workspaces.
  - A workspace can turn a granted skill off with `purlis skill grant <name> -w <workspace> --off`. It can't turn on one it wasn't granted.
  - A workspace can keep skills of its own.
  - Every level can keep local skills that stay on your machine. `purlis skill share` or the studio's Share gives one to the team.
  - The studio shows the skills of a chat in any workspace, with one line for each skill left out. Settings › Workspace › Skills counts them and opens the studio (#2767).

### Changed

- **A LIVE workspace's `skills/` is committed with it**, as its memory and todos are. `purlis init`, `purlis reinit` and every save keep each level's `skills.local/` out of git (#2767, #2874).
