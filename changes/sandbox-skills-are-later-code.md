### Security

- **A sandboxed chat can no longer author or edit a project's skills at all.** In every
  harness, a sandboxed chat may never write `.claude/skills/`, `.agents/`, or opencode's
  `tui.json` and `tui.jsonc`, at any depth. These join the other harness config it already may
  not write. Each of them can make a later chat, run outside the sandbox, start code (#1057).
