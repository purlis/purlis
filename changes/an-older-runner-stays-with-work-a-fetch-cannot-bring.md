### Changed

- **An older runner of a pool stays while it holds work purlis can't bring back on its own.**
  Before it goes after an update, purlis asks it for every repository it holds and checks each
  against the project's folder here, as it already does for a task's own runner. A stash, a
  commit on no branch, a clone made only on the runner, or one whose clone or workspace was
  removed or renamed here keeps it, and so does work of a project this machine no longer knows.
  Settings › Pools lists what keeps it, and says how to keep that work or let it go.
