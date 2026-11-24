### Fixed

- **A push the route refuses now says why in git's own output.** A chat's push to a branch that is
  not its own, or from a stale tip, used to print `RPC failed; HTTP 403` and "Everything up-to-date",
  which read as a forge permission fault. It now prints `! [remote rejected] <branch> (purlis: …)`
  with the reason, and a refused `info/refs` prints `fatal: remote error: purlis: …`. Nothing is
  sent to the forge either way (ADR 0128, #7363).
- **A chat can push the branch a task it dispatched made.** After the task ends, the chat that asked
  for the work may push its follow-up from the tip the task left; from then on the branch is that
  chat's and the task's. A handoff, an unrelated chat and a stale tip are still refused (ADR 0128,
  #7363).
