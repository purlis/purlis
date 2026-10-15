### Changed

- **A sandboxed Claude Code chat takes its settings from purlis and from you, not from files
  in the project.** At each start of a sandboxed Claude Code chat, full sandbox or floor, purlis
  reads the project's `.claude/settings.json` and `.claude/settings.local.json`. They are loaded
  as they are while they hold only deny and ask rules, plugins, model, output style or theme.
  If they hold anything else, or cannot be read, the chat starts on your own Claude Code
  settings and purlis's. The project's deny and ask rules still apply. The chat's tab names
  the keys that were not applied, and says that the project's CLAUDE.md, rules, skills, agents
  and commands are not loaded in that chat either. A chat with no sandbox is unchanged.
  Commands and the Edit tool in a sandboxed chat do not write your own Claude Code settings
  files, at the floor or with the sandbox fully on.
