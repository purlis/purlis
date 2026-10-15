### Added

- **The team decides how far its people may turn the sandbox down.** In `charter.toml`,
  `[sandbox] policy = "never"` keeps every chat in the full sandbox: "Start without the sandbox"
  and a lower level of your own are refused, with a sentence naming the policy.
  `policy = "local"` (the default) leaves each person their own level.
- **`[sandbox] mode = "off"` turns the sandbox off for the team, by consent.** It no longer reads
  as on with a refusal: each person is asked once, "Your project turned the sandbox off (commit
  <sha> by <author>)", with **Accept** and **Keep it on for me**. Until they answer their chats
  keep their own level; once they accept, a chat with no level of theirs runs with no sandbox
  from its next start. Settings › Sandbox › Your level shows the answer and offers the other.
  Under `policy = "never"`, `mode = "off"` is refused (#2071).
