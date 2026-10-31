### Changed

- **Signing a profile out of a pool reaches its draining runners too.** Sign out in Settings › Pools
  now also runs on any runner of the pool still finishing its chats after an update, says which
  it reached, and nothing carries that sign-in across when the runner finishes.
- **A drained runner's sign-ins are carried once.** It is unpaired first, and a removal that failed
  only tries the removal again.
- **opencode signs out on a pool's runner from its line.** Sign out in a terminal opens a shell tab
  where opencode asks which sign-in goes.
