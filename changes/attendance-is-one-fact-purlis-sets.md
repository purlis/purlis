### Changed

- **Whether a chat is attended is one fact purlis sets.** A chat purlis started as a task, a loop
  or a runner is unattended, and so is a Claude Code chat whose hook reports a permission mode
  purlis didn't start it in. Claude Code's mode is read only from the chat's armed hook, by the
  strings measured on Claude Code 2.1.325, and a string nobody measured never reads as attended.
  Codex's attendance comes only from how purlis started the chat. Once unattended, a chat stays
  so. Nothing reads the fact yet; what reads attendance today is unchanged (#6408, #3825).
