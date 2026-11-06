### Fixed

- **A skill that names another one the router hid now starts it, even when the other one narrows
  the chat.** On Claude Code, `skill_load` used to send the chat to `skills:<name>` for a skill that
  takes tools away or runs in a fork. When the harness hadn't been handed that skill, it answered
  "Unknown skill", so nothing could start it. The same happened for a skill installed after the
  chat started. Now such a skill is loaded and the chat is told its narrowing isn't applied. A
  skill the harness was handed is still started by the harness, with its narrowing.
