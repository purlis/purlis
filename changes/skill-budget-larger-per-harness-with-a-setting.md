### Added

- **Set a chat's skill budget.** `settings.skills.budget` in a project or a workspace overrides how many skills a chat lists and how many characters they take, from Settings › Skills. A value past a harness's own limit says what that harness would drop, and is never refused (#4610).

### Changed

- **Claude Code chats list 60 skills and 15,000 characters** where they listed 30 and 6,000, counting names and whole lines as Claude Code does. Codex and opencode stay at 30 and 6,000 (#4610).

### Fixed

- **A Claude Code chat on a 200k window keeps its skills' descriptions.** Claude Code's own entries used to fill its limit and every plane skill's description was dropped. purlis now hands each chat a limit that holds them (#4610).
