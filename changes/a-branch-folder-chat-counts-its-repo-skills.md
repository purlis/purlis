### Fixed

- **A chat in a branch folder counts the repo skills of its branch** (Skills, #4238). Such a chat
  used to count no repo skills at all, so a skill on that branch did not take the place of a
  plane skill of the same name, and the budget counted both. It now counts the ones its harness
  finds in the branch folder, under the name of the clone the folder was cut from.
