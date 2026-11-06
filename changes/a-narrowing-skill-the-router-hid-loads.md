### Fixed

- **A skill the router hid now loads on Claude Code when it narrows the chat.** Before, the chat
  was told to start it with Claude Code's own Skill tool. That tool didn't know the skill, so it
  answered "Unknown skill" and the skill never ran. A skill installed after the chat started had
  the same problem. Now the skill loads, and the chat is told its narrowing isn't applied. A skill
  the router kept still starts through Claude Code, with its narrowing.
