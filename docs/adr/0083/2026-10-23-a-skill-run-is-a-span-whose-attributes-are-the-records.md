# Amended for SK-20 (2026-10-23): a skill run's attributes are on its records, not the resource

§6 puts the attributes purlis sets on the resource, as a chat's place and persona hold for its
whole life. A skill run does not. The Skill studio (ADR 0099, spec #2762, decision 9) makes every
use of a skill a span inside the chat's run, from its start to the end of the turn, and a chat
runs many skills, one inside another. The spec asked for the skill as a resource attribute
(OB-18). This amendment records why it can't be one, and what the exporter reads instead.

- **The skill is on each record, as the event log already carries it.** Every event the log's
  writer writes inside a skill run has `skill` and `skill_run` in its body, the innermost run's
  (SK-3, `docs/plane-format.md`). A resource attribute is fixed for every record a resource
  sends, so it can't change from one span to the next. The exporter therefore writes these as
  attributes of each log record and of the span:

  | Attribute | Value | From |
  |---|---|---|
  | `charter.skill.name` | the skill, as the harness named it | the body's `skill` |
  | `charter.skill.run.id` | the skill run's ULID | the body's `skill_run` |
  | `charter.skill.run.parent_id` | the run it started inside, absent at the top | `skill.started`'s `parent_skill_run` |
  | `charter.skill.started_by` | `skill-tool`, `read`, `mcp-load`, `use-skill`, `curation` or `trigger` | `skill.started`'s `started_by` |
  | `charter.skill.level` | the copy's level (`project`, `local workspace`…), absent where not known | `skill.started`'s `skill_level` |

- **A skill run is a span of its own.** It starts at its `skill.started` and ends at its
  `skill.ended`, and it is the child of the chat run's span (`charter.run.id`) or of its parent
  skill run. Its signals (`tool_calls`, `tool_errors`, `refusals`, `asks`, `tokens`, `ms`) are
  the `skill.ended` body's. They are signals, never a pass or a fail (decision 24).
- **The names take §6's prefix, `charter.*`,** as §6 still spells it: no amendment has renamed
  §6's attributes to `purlis.*`. Should one, these move with it. They freeze with §6 when OB-18's
  dashboard pack ships. Until then a rename is a change to this record.

## Decided in implementation (SK-20)

- **D-SK20a: the exporter is not built here.** There is no OTel exporter in code yet. SK-20 puts
  everything the table above reads on the event log's entries, and the OTLP export lands with
  OB-18. Rejected: a mapping function written now with no caller but its tests.
- **D-SK20b: `skill_level` is added to `skill.started`.** The start row of the plane's skill
  log already had the level. The event log didn't, and an exporter reads only the event log.
