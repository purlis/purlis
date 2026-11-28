### Added

- **How long a chat rests before purlis hibernates it is yours to set, per project.**
  Settings › Project › Chats › Hibernate after is `[chats] hibernate_after` in the project's
  `charter.local.toml`: 30 minutes unless you set it, a time such as `45m` or `2h` of at least
  10 minutes, or `off`, which hibernates none of the project's chats by themselves however long
  they rest. Each chat rests its own project's time, over one count of running chats for the
  device, and a change takes effect at once. Hibernate on a chat's row is not changed by it
  (SC-4h, #7637).
