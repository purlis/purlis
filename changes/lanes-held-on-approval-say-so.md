### Fixed

- **A dispatched task held on an approval prompt says so.** Its row in `purlis dispatch list`
  used to read only "waiting on the person", which a coordinator could not tell from a question
  already answered. It now reads "waiting on the person to allow a tool call in its pane; it
  reads messages sent to it once that is answered" (#5771).
- **A sandboxed chat's `purlis worktree add` no longer tells it to enter the new folder.** Its
  sandbox cannot write there, and the advice sent a dispatched task to an approval prompt nobody
  was watching. It now says the folder is out of its reach, and names what works: open a chat on
  the folder, or have the chat that asked dispatch a task with `--in worktree` (#5770).
