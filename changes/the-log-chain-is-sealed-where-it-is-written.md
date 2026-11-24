### Fixed

- **The event log's hash chain is now sealed, and a rewrite of it is caught.** The chain that
  records a chat running with the sandbox off was never sealed in the usual setup, because the
  session host that writes the log did not hold the seal key; it now does. The seal covers the
  chain's start as well as its head, so deleting or rewriting the lines before the end, deleting
  the log, or stripping the chain from it each read as a break. A chain starts for any chat that
  runs with no sandbox, including one a project or template set to level off. Each chat's start
  checks the log and shows a Notice if it was changed, and a log opened again after tampering
  is not sealed over. Doctor shows a warning, never OK, for a chain it could not check against a
  seal, and says on Linux and Windows that the seal is only as strong as the keyring.
