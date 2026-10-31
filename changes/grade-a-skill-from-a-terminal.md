### Added

- **Grade a skill from a terminal.** `purlis skill grade <name> --run` runs a grade in the window
  that has the project open, with the same plan and caps as pressing Grade. It asks before it
  spends (`--yes` skips the question for a script), follows the grade and prints the scorecard.
  Ctrl-C stops it. A chat cannot run it (SK-61, #3241).
