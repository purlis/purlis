### Fixed

- **A task no longer loses its shell when your Claude Code settings hold auto mode deny rules.**
  With `autoMode.soft_deny` or `autoMode.hard_deny` rules in your own or your managed Claude Code
  settings, Claude Code refuses every call auto mode's check cannot judge, without asking you. A
  task purlis would start in auto mode, from a chat in auto mode or with its prompts off, now
  starts in Claude Code's default mode and asks you, and its pane says why from its start
  (#6315).
