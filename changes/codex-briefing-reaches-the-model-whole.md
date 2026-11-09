### Fixed

- **A Codex chat is told its whole start briefing.** Codex keeps 10,000 bytes of what one hook
  hands it and cuts the middle, and in an ordinary workspace the briefing was longer than that,
  so a Codex chat lost part of its persona's memory, its sandbox notes or the reports kept for
  it. A Codex chat now gets the briefing from four start hooks: who and where it is, how it
  works here, its skills, and what others said to it. Codex asks once to trust the two new
  hooks. Claude Code's and opencode's briefings are unchanged (#5935).
