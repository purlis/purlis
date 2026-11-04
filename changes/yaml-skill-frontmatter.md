### Fixed

- **A skill's description written as YAML reads as its words, not `>`.** A folded (`>`), literal
  (`|`), quoted or multi-line `description:` in a `SKILL.md` is read as YAML reads it in the
  Skill studio, the Skills panel, the skills listed to a chat and the skill budget, so such a skill
  is listed by what it says. A rename or copy keeps it whole, and the studio says when a
  description cannot be read (#5182).
