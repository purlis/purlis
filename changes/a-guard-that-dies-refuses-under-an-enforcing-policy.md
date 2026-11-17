### Security

- **A guard that dies no longer lets a tool call through where your project enforces what it
  checks.** In a Claude Code chat, a purlis guard that timed out, crashed or could not start used
  to let the command, read or edit run. Now Claude Code refuses the call wherever the project's
  `[policy]` puts a family that guard judges at `enforce`. Under `watch`, the default, that is a
  dead Bash or Read guard, because `watch` enforces `secrets` and `guards.credential`. Under
  `open`, or where those families are at `log`, the call still runs. Codex has no such option, so
  a guard that dies there still lets the call run.
