### Changed

- **A prompt purlis fills in now starts its skill directly.** In Claude Code, the Skill studio's
  prompts, Try it, the Curate menu and the theme chats type
  `/purlis:purlis-skill-studio write outcome evals for login in skills/login`, unsent, in place of a
  sentence naming the skill. opencode gets its own `/<name>` form, and Codex keeps the sentence,
  because purlis only lists skills there (#5186).
