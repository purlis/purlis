### Fixed

- **A persona's tool declared through a link does what its declared name says.** A tool runs
  the program its links resolve to, and that program now starts under the name the tool was
  declared as. A tool `unxz`, whose link resolves to `xz`, unpacks again instead of packing,
  and the same goes for `gunzip` and `git-*` links. Which program runs and which credential it
  is handed are unchanged (#1977).
