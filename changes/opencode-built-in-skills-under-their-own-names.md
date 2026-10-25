### Changed

- **opencode gets purlis's built-in skills under their own `purlis-` names.** In an opencode
  chat, purlis's skills are `purlis-secrets`, `purlis-handoff` and so on, as Claude Code knows
  them as `purlis:secrets`. A skill of a repo's or your own named `secrets` is then another skill
  beside purlis's, each reached by its own name. _Use skill…_ types `/purlis-secrets`, and a
  chat's own list of its skills names them the same way. A skill of the project's, a repo's or
  your own named like one of them, such as `purlis-secrets`, is listed as left out and says to
  rename it. Claude Code and Codex chats are unchanged.
