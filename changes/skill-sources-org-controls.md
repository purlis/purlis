### Added

- **An org's project controls where its skills come from.** A `[skill_policy]` table in
  `charter.toml` keeps an allow and a block list of skill sources and single skills, and a
  minimum age for a commit (7 days by default, with exact commits exempted by a reviewed
  change). The Skill studio's review and `purlis skill install | update` say why a skill is
  refused, and every install, update and removal is recorded in the event log the audit is
  written from (SK-11, #2774).
