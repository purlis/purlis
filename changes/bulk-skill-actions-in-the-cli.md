### Added

- **Change several skills at once from the command line** (#5357). `purlis skill rm`, `off`,
  `on`, `pin` and `archive` take several names, or `--source <url>` for every skill installed from
  a source, and make the change as one commit after printing its review and asking you at your
  terminal. A chat can't make such a change, and neither can a script with no terminal: the
  command writes nothing and puts the change in front of you as a proposed change, which you
  apply or dismiss in the window.
