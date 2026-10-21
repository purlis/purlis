### Added

- **Try the Skill studio while it is built.** Settings › You › Previews has a *Skill studio*
  switch for this machine. With it on, every chat gets the project's own skills (a folder in
  `skills/` at the project's root, for chats at the root), on Claude Code, Codex and opencode,
  and the palette's *Open Skill studio* lists every skill a chat gets and why any other is left
  out (#2765). Each time a chat uses a skill, its tab shows the skill and the tool calls made
  so far until the turn ends, and the skill's own tab lists every run with what happened inside
  it: tool calls, errors, refusals, asks, tokens and time (#2766). `purlis skill runs` prints
  them.
