### Changed

- **Clones purlis has no record of are one ask.** However many clones you made in your own
  terminal, the project lists one _Trust these clones_ in Asks, and nothing is trusted until you
  answer it. `purlis doctor` lists the same clones in its new `unrecorded clones` row, with the
  address each remote names, and names a clone whose remote has several addresses with the fix.
  A clone a chat made or can write is never in the ask, and a clone a chat already asked you to
  trust is asked for once. `purlis doctor --fix move-into-repos` moves clones and trusts no new
  one: it names the clones it moved that purlis has no record of, and says to trust them in
  purlis's window, in Asks › Trust these clones. A clone purlis already trusted stays trusted
  after the move while its remote is unchanged.
