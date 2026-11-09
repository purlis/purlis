### Added

- **A persona can bring another workspace's skill.** Write `workspaces/<ws>:<name>` on its
  `skills:` line: its chats in every workspace get that workspace's skill. The persona's own copy
  still wins. The named copy beats one of the same name in the chat's own workspace. With the
  Skill studio flag on, the studio as the persona lists it as `workspace <ws>`. `persona lint`
  warns of a workspace or skill that is not there (SK-5a, #2933).
