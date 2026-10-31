### Fixed

- **A chat in a branch folder counts the repo skills of its branch** (Skills, #4238). Such a chat
  used to count no repo skills at all, so a skill on that branch could not narrow a plane skill
  of the same name or count toward its budget. It now counts the ones its harness finds in the
  branch folder, under the name of the clone the folder was cut from.
