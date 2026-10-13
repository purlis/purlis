### Changed

- **A task's Changes tab lists the paths purlis will not read.** A path a branch changed whose
  folders include one called `.git`, or whose name is not a plain path, used to be left out
  without a word. It is now listed under its place with its mark and the reason. It is drawn
  as text: nothing opens, compares or stages it (#704).
