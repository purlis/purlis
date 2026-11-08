### Changed

- **purlis's built-in skills, a chat's briefing and purlis's tool descriptions use one
  vocabulary.** They say the person (never the user or the operator), helper (never sub-agent),
  branch folder, repo and chat, as `CONTEXT.md` does, and the shipped-skill check fails on
  "control plane", "sub-agent", "the user" and "operator" outside a tool's name, and on a skill
  named in prose that purlis does not ship (#5723).
