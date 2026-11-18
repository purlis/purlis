### Added

- **Known gaps are declared, and the chat still starts.** Where a rule in force is one a harness cannot
  enforce on this system, such as a hook Codex does not have, Claude Code's reads or the missing
  sandbox on Windows, purlis says so in Settings (*Sandbox › Known gaps*), in `purlis doctor`
  for a project that wrote a `[policy]`, in the chat's start event and on its pane. A project that
  wants such a start refused writes a forbid on `start` over `context.gaps`; the shipped default
  has none (M75-37, #7046).
