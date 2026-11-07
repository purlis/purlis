### Fixed

- **A write to your workspace files lands in the folder purlis checked.** New file, new folder,
  rename and import keep the folder they checked open until the write. If a chat swaps that folder
  for a link in the meantime, nothing reaches the link's target. A delete is refused if its folder
  changed after the check. Renaming a file onto another hard link of itself is refused and no
  longer reports a success that did nothing. Importing a FIFO is refused at once rather than
  hanging. An import that stops partway names the files it already copied. On Linux, a delete is
  refused for an entry on another disk than your Trash, since the system would copy it there and
  lose its permissions and links (#5551).
