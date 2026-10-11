### Fixed

- **A chat over ACP reads its agent on a thread of its own.** A process that left an agent's
  process group and kept its output open used to park a thread of a pool every chat shares, so
  enough of them could slow every chat's reads. Now it holds only its own chat's reader, which
  ends once that process lets go. A tool call's kind and status, and a turn's stop reason, are
  purlis's own words, and a stop reason purlis does not know no longer reads as a cancel. When
  purlis restarts itself to leave the terminal it was started from, and the first process is
  killed in the instant before the second one checks, the second one now ends too, rather than
  run on as a window nothing started (#1117).
