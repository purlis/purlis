### Changed

- **`purlis skill install | update | rm` works at your own terminal on an org's project.** The
  session host records each change in the event log before it is made, as yours at the
  terminal, so a project with `[skill_policy]` no longer sends you to the Skill studio. It needs
  purlis or `purlis serve` running (SK-11a, #6030).

### Fixed

- **The Skill studio installs, updates and removes skills on an org's project again.** The
  event log refused the audit of each change, so the change was refused too (#6030).
