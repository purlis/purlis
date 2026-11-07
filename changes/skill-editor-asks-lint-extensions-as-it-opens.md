### Changed

- **The skill editor asks lint extensions as an edit opens, not as you type.** Each extension
  that declares `skill-lint` starts its program confined for every question, which took 345 to
  449 ms on macOS, longer than the 200 ms pause after a keystroke the live lint waits. purlis's
  own lints still check the text as you type; what an extension said of the text as it opened
  stays shown beside them, and the studio asks the extensions again once the skill is saved
  (#3093).
