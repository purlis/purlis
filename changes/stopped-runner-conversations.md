### Fixed

- **An older runner's chats' conversations are no longer lost when it had already stopped.** After
  an update or a Replace, a runner that had stopped first, for a suspend or because its container
  was removed outside purlis, was removed with its home and every conversation its chats left
  there. Now purlis looks in that home first: where it holds a conversation, the runner is kept,
  and Devices offers Bring back conversations, which puts them in the pool's runner so those chats
  pick up there, or Discard…, which asks twice. A home with no conversation goes as before.
  opencode's conversations are kept but not brought back. Replace no longer keeps a runner already
  stopped whose home holds none.
