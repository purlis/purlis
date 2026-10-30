### Changed

- **A skill's History and Grades follow it across a rename and a move** (#3918). History lists
  the versions from before, read from the folder and name the skill had then, with their runs
  and grades, and the version that renamed or moved it shows that as its diff. Its Grades and
  each grade's scorecard read the grades recorded under its earlier name or level, and the
  chats this machine kept of its grades go with a rename. A revert goes back only to a version
  of the folder it has now, and says so. The Rename and Move dialogs say its history and grades
  move with it; for a skill only on this machine, which git keeps no history of, they say they
  start again.
