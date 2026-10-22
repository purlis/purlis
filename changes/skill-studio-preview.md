### Added

- **Try the Skill studio while it is built.** Settings › You › Previews has a *Skill studio*
  switch for this machine. With it on, every chat gets the project's own skills (a folder in
  `skills/` at the project's root, for chats at the root), on Claude Code, Codex and opencode,
  and the palette's *Open Skill studio* lists every skill a chat gets and why any other is left
  out (#2765). Each time a chat uses a skill, its tab shows the skill and the tool calls made
  so far until the turn ends, and the skill's own tab lists every run with what happened inside
  it: tool calls, errors, refusals, asks, tokens and time (#2766). `purlis skill runs` prints
  them. A chat can also list, find and load its own skills and see their recent runs, so a
  chat program that has no skills of its own still gets yours. Loading a skill counts as a run,
  and the run says the chat loaded it (#2776).
  *Use skill…*, in the palette, on each chat tab's menu and on the Skills chip of a chat's
  pane, lists exactly that chat's skills, asks for the arguments a skill declares, and types the
  skill into the chat for you to send, in the way that chat's tool expects it, on Claude Code,
  Codex and opencode alike. The chip and the tab's menu show how many skills and how many
  characters of their descriptions the chat lists, and warn at four fifths of purlis's budget;
  a manual-only skill costs nothing where the chat leaves it out, as Claude Code does (#2775).
- **A persona brings its skills into every workspace** (Skill studio preview). A persona's own
  skills, and the skills its definition names, reach every chat as that persona, in any
  workspace, whatever the workspace was granted; the persona's copy wins over a workspace's or
  the project's of the same name. Its page counts them and opens the Skill studio on a chat as
  the persona, where you add a skill by name or remove one. A chat whose persona names a skill
  this machine does not have shows a Notice naming it, and starts without it. Names written the
  old way, `charter:<skill>`, still mean purlis's own skill (#2768).
