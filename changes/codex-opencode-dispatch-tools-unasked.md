### Changed

- **Codex and opencode chats dispatch and follow their tasks without their harness's prompt.**
  As in Claude Code, each dispatch tool is allowed by name, as Codex's `approval_mode =
  "approve"` and opencode's `permission` rule: consent to a dispatch is purlis's own dispatch
  grant. Your own rules still win: an approval you set on purlis's server in Codex's config, or
  an opencode permission rule that names the tools or asks about every tool, is kept (#3794).
