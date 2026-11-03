### Fixed

- **The Skill studio's chats open again with their prompt typed.** "Ask a chat to write outcome
  evals" never opened a chat: its prompt was longer than the 800 characters Claude Code draws a
  paste whole, so purlis typed nothing. "Improve from runs…", the trigger tests' "Ask a chat to
  write them" and "Ask a chat to draft a skill" ran over too for a long name. Each now types one
  short line, naming the job, the skill's folder and the new built-in `purlis-skill-studio`
  skill, which holds the steps and the formats (#4714).
