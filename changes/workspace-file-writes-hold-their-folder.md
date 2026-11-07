### Fixed

- **A write to your workspace files lands in the folder purlis checked.** New file, new folder,
  rename and import keep the folder they checked open until the write. If a chat swaps that folder
  for a link in the meantime, nothing reaches the link's target. A delete is refused if its folder
  changed after the check. Renaming a file onto another hard link of itself is refused and no
  longer reports a success that did nothing. Importing a FIFO is refused at once rather than
  hanging. An import that stops partway names the files it already copied. A rename is refused on
  a disk that cannot rename without the chance of replacing a file (NFS, some FUSE and SMB mounts).
  On Linux, a delete on another disk than your Trash goes to that disk's own Trash, and is refused
  only when that disk has none purlis can use, since the system would then copy it into yours and
  lose its permissions and links (#5551).
