### Added

- **A workspace or a persona chooses the router its chats' skills are listed by** (Skill studio
  preview, #3230). `settings.skills.router` in a workspace's `workspace.json`, or
  `skill-router:` in a persona's definition, names an approved extension that declares
  `skill-router`; the persona's choice wins, and the built-in router stays the default. The
  router is asked at each chat's start which skills to leave out of its list, each with its own
  reason, and has 100 ms: past that, on an error, or on an answer that would hide a pinned skill,
  a chat lists what the built-in router shows. Use skill… and the Skill studio say how long the
  router took and why a chat fell back, and the chat's event log keeps a `skill.routed` record
  that each skill run names.
