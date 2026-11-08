### Added

- **purlis now notices when a skill names another skill bare, as in `Use the Skill tool with
  "grilling"`.** `purlis skill uses` lists it as a hint, the skill lint suggests writing
  `skills:grilling`, and installing the skill brings `grilling` along from the same source. A
  name counts only when it is quoted or in backticks, its line says "skill", and a skill of that
  name exists.
