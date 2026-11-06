### Fixed

- **A save request that conflicts with its target now recovers by itself** (#5338). In the
  `pr` and `pr-merge` modes, a save and the app's own look bring a target that moved into the
  plane as one merge commit where the request needs it, and the request is pushed so it can
  merge. A request that merges as it is gets no merge commit. Session indexes, the
  todos index and the `## Sessions` line are rebuilt from the records and todos, so nothing is
  lost and the count stays true. A conflict in something a person wrote stops the save and names
  the file with nothing changed, and Resolve in the Saving view settles it in one stop. Saved
  planes pick up the new merge rules with their next save.
