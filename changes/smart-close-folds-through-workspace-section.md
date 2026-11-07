### Fixed

- **A smart close and a memory compact no longer stop on a sandbox block.** Both skills now fold
  lasting decisions, terms and goal changes into `workspace.md` through the `workspace_section` and
  `workspace_vision` tools, which purlis writes for the chat, in place of telling a sandboxed chat
  to edit the file itself (#5720).
