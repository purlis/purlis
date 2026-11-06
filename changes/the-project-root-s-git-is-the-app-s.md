### Changed

- **A chat leaves the project root's git to the app.** A Claude Code chat no longer writes the
  project root's `.git`, wherever in the project it stands: `git add`, `commit`, `fetch`, `stash`
  and `rebase` there stop, and read-only git works as before. `purlis save` in a chat the app
  started is run by the app, with the same `-m`, `--sign` and `--no-push`, and its commit names
  the chat. That save leaves out files the chat cannot read, the commit's trailers are the
  app's, and it brings in nothing from the remote: `--pull` is refused, a remote that moved
  leaves the commit unpushed until `purlis worktree update`, and a project that saves through a
  pull request is committed and pushed by your own save. Codex, opencode and Claude Code on a runner save the project with
  their own git, as before. A skill change or a workspace rename a chat makes is saved by the
  next save. Your own terminal works as before.
- **The app runs git only in a git folder whose files are its own.** A fetch or an update of a
  clone stops, naming the file, where `packed-refs` or a file under `logs/` is a link, has a
  second name or cannot be read. A save, update, push or resolve of the project stops the same
  way for those and for the files git keeps beside them, such as `COMMIT_EDITMSG`, `ORIG_HEAD`
  and the merge and rebase state. The app's fetches run no automatic maintenance.
