### Changed

- **With the workspace picked, Explorer shows the workspace's folder.** Its own files sit beside
  its repos, and each repo's row is where the repo is on disk: in `repos/`, or at the top for a
  repo cloned before. One repo still opens at once. What purlis keeps there is marked *managed*
  and opens read-only, with a link to its own view (Memory, Todos, Sessions, or the vision for
  `workspace.md`). Dotfiles show with **Show hidden**. The filter finds among the workspace's
  files, and with one repo it covers those files and the repo (#5375).
