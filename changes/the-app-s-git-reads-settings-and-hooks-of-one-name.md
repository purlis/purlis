### Changed

- **The app runs git only where the repository's settings, hooks and index have one name.** A
  save, update, push or resolve of the project, and a fetch or an update of a clone, stops,
  naming the file, where `config`, `HEAD`, `index`, a hook, a file under `info/` or
  `objects/info/alternates` has a second name (a hard link). A hook or a setting that is a
  link is left to git as before. Where the refs or the reflogs are a link to another
  repository's folder, as `git-new-workdir` makes, the save says so and that
  `git worktree add` shares a repository without one.
- **A push the app makes for a chat is never signed**, whatever `push.gpgSign` says: a chat's
  save of the project and its `purlis worktree push`.
