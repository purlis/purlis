### Fixed

- **A manual-only skill that narrows the chat now loads when you name it in a Claude Code
  chat.** The chat used to be told to start it as `skills:<name>`, which Claude Code refuses for
  a manual-only skill, so it stopped there. purlis now loads it and says its narrowing is not
  applied. Type `/skills:<name>` yourself to run it with its narrowing (#5880).
