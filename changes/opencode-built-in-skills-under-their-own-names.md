### Changed

- **opencode gets purlis's built-in skills under their own `purlis-` names.** In an opencode
  chat, purlis's skills are `purlis-secrets`, `purlis-handoff` and so on, as Claude Code knows
  them as `purlis:secrets`, and the index skill a chat past its skill budget gets is
  `purlis-skill-index`. A skill of a repo's or your own named `secrets` is then another skill
  beside purlis's, each reached by its own name. _Use skill…_ types `/purlis-secrets`, and a
  chat's own list of its skills names them the same way. A project skill named like one of them,
  such as `purlis-secrets`, is left out and says to rename it. A repo's or your own of that name
  still loads beside purlis's, and the Skill studio says the chat may use either one. Claude Code and Codex chats are unchanged.
