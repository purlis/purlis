### Changed

- **The skill editor asks lint extensions as an edit opens, not as you type.** Each extension
  that declares `skill-lint` starts its program confined for every question, which took 345 to
  449 ms on macOS, longer than the 200 ms pause after a keystroke the live lint waits. purlis's
  own lints still check the text as you type, and what an extension said of the text as it
  opened stays shown beside them. Once the skill is saved, the Skill studio's list asks the
  extensions of the saved skill, within its own time, as it does for every skill (#3093).
