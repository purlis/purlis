### Changed

- **A chat leaves the project root's git to the app.** A Claude Code chat no longer writes the
  project root's `.git`, wherever in the project it stands: `git add`, `commit`, `fetch`, `stash`
  and `rebase` there stop, and read-only git works as before. `purlis save` in a chat the app
  started is run by the app, with the same `-m`, `--sign`, `--no-push` and `--pull`, and its
  commit names the chat. That save leaves out files the chat cannot read, and the commit's
  trailers are the app's. Codex, opencode and Claude Code on a runner save the project with
  their own git, as before. A skill change or a workspace rename a chat makes is saved by the
  next save. Your own terminal works as before.
- **The app's git moves no ref through a link in a clone's git folder.** A fetch or an update
  stops, naming the file, where `packed-refs` or a file under `logs/` is a link, has a second
  name or cannot be read, and the app's fetches run no automatic maintenance.
