### Changed

- **Every refusal by the sandbox is now a block in the chat's event log, not only a held
  connection.** A write refused outside the chat's folders, a service or socket on this machine,
  a host you keep blocked, and a refusal of what the sandbox always protects (vaults, purlis's own
  state, files later programs load) each give one block from the sandbox. The block names the
  host, the service or socket, or the folder by its path inside the project; a folder outside the
  project is not named. A refusal that is asked about stays an ask in Asks until you answer it.
  It becomes a block when you keep it blocked, when a newer ask of the same kind takes its place,
  or when the chat ends. An Allow writes no block. A refusal nobody can allow is a block at once.
  A refusal reported by a Codex or opencode chat's own hook is marked unverified, since the chat
  can reach that hook. An unnamed refusal heard while the same refusal is asked by name is now
  one ask, not two (ADR 0124, R-222, R-353).
