### Added

- **`purlis theme list` and `purlis theme check` know every theme level.** `list` shows your
  theme folders, the project's and each workspace's beside the built-in and extension themes,
  each with its level, and `check machine:<folder>`, `check project:<folder>` or
  `check workspace:<ws>:<folder>` checks a level's folder exactly as the window reads it. The
  check also says which consent floors the window holds when it draws the theme, and which
  tokens it draws as purlis's own for them (#2518).

### Changed

- **`purlis theme check --json` is not `clean` when a consent floor is held.** A theme the
  window draws with purlis's own values for a floor it breaks now answers `clean: false`
  (#2518).
- **`purlis theme list` names your own `theme.json` by its level.** Its FROM reads
  `machine <path>` (#2518).
