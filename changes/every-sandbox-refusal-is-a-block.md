### Changed

- **Every refusal by the sandbox is now a block in the chat's event log, not only a held
  connection.** A write refused outside the chat's folders, a service or socket on this machine,
  a host you keep blocked, and a refusal of what the sandbox always protects (vaults, purlis's own
  state, files later programs load) each give one block from the sandbox. The block names the
  host, the service or socket, or the folder by its path inside the project; a folder outside the
  project is not named. It names the tool of the call that was refused, not a later one. A
  refusal that is asked about stays an ask in Asks until you answer it. It becomes a block when
  you keep it blocked, when a newer ask of the same kind takes its place, when the chat ends, or,
  for an ask the chat's own hook raised, when the chat starts its next turn without your answer.
  An Allow writes no block. A refusal nobody can allow is a block at once. A refusal a chat's own
  hook reported is marked unverified on every harness, since the hook reads what the command
  printed, which the chat can write. Blocks written by your Keep blocked or by the chat's end are
  not counted toward pausing a chat that keeps hitting the same block. An unnamed refusal heard
  while the same refusal is asked by name is now one ask, not two (ADR 0124, R-222, R-353).
