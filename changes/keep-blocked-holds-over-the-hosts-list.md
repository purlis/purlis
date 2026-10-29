### Fixed

- **Keep blocked holds for this chat even for a host the project's hosts list names** (#3407).
- **Keep blocked also takes back the chat's own Allow of that host.** It leaves the Granted list,
  the chat's next start no longer has it, and the chat's browsers end, as when an Allow is
  removed (#3407).
