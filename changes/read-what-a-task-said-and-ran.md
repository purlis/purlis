### Added

- **A chat can read what its task said and ran.** `purlis dispatch read <chat>` prints a task's
  last three messages and its last twenty shell commands with their exit codes, as its harness
  handed them to purlis, quoted as data with credential shapes masked. Only the chat that
  dispatched the task can read it, while it runs and after it ends. The whole conversation stays
  yours, in the task's tab (#1981).
