### Changed

- **Clones purlis has no record of are one ask.** However many clones you made in your own
  terminal, the project lists one _Trust these clones_ in Asks, and nothing is trusted until you
  answer it. `purlis doctor` lists the same clones in its new `unrecorded clones` row, with the
  address each remote names. `purlis doctor --fix move-into-repos`, run in your own terminal,
  asks whether to trust the clones it moves that purlis has no record of, and a clone purlis
  already trusted stays trusted after the move.
