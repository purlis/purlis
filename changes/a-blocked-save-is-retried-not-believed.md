### Fixed

- **A project blocked by an old "conflict" clears itself.** Save on a project whose last push
  stopped now pushes again even when nothing changed, so the block either goes or comes back
  naming the files that conflict. A conflict record that names nothing reads "Save stopped
  earlier — Save to retry" and is retried. A successful Update clears the block its replay
  settled.
- **A save's conflict is never sent round in a loop.** The Saving view names the conflicting
  files and the git commands that settle them, with _Open terminal here_, and no longer offers a
  chat, which may not rebase the project's root. Update's conflict answer names the same
  commands.
