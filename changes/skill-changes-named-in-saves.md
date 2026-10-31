### Changed

- **A save names the skills it changed** (Skill studio preview, #2876). A save's message counts a
  skill's files under the skill's name, `skill deploy 2`, `ops skill review 1`, where before they
  were counted as the project's, a workspace's or a persona's files. The window tells its readers
  a skill changed as its own kind of change, so the Skills view and a persona's page read again
  for it, and a workspace's todos and memory do not.
