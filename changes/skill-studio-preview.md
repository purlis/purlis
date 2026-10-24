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
- **The Skill studio lists each program's own skills and plugins** (#2769): Claude Code's,
  Codex's and opencode's, in your own folders, in their plugins, and in a workspace's repos,
  each with whose it is. Each can be turned off for this project's chats where the program
  allows it, and says plainly where it does not. A project skill that a repo or a program also
  has says that a chat gets both. With the studio on, Settings' Plugins group is a summary that
  opens it.
- **A skill can only narrow a chat** (Skill studio preview, #2771). A skill that asks to
  pre-approve tools or to add commands to a chat's events is handed to the chat without them,
  and says so; one that takes tools away keeps that. Each skill in the Skill studio shows, for
  Claude Code, Codex and opencode, whether everything it asks for applies there, and what is left
  out, carried over or not applied. A skill of a program's own or a repo's that would widen a
  chat is marked. A manual-only skill now stays out of the chat's choice, and costs nothing in
  its budget, on all three. The editor's live lint also warns of a description too short to say
  when to use the skill, a skill it names that does not exist, and a program the skill runs that
  this machine does not have. A chat that starts without a skill it would have loaded, because
  the skill is broken, says which and why on its tab, with a way to the Skill studio.
- **Move your project's skills into the Skill studio in one step** (Skill studio preview,
  #2784). `purlis doctor --fix project-skills` moves every skill in the project's
  `.claude/skills` into its `skills/`, shared with every workspace as before. It rewrites the old
  `charter:<skill>` names to `purlis:<skill>`, but never inside quoted code or an image name. It
  all goes in one commit that `git revert` undoes. It checks first that every chat keeps its
  skills, and changes nothing if one would lose one. From then on every chat in the project gets
  those skills, the Skill studio switched on or off, and Codex chats get them too. Clones stop
  getting a copy of `.claude/skills`. Your clones lose their copies at once and a teammate's the
  next time a chat starts there, while a copy someone edited is kept and named. It says what it
  changed, and which skills a persona names that no chat can get. It needs the Skill studio
  switched on to run, and refuses while the project has changes that are not committed or its git
  is in the middle of something.
- **A skill's history, state, owners and uses** (Skill studio preview, #2782). A skill's tab
  lists every saved version of it, newest first, with who saved it, the runs that started on it,
  what it changed, and *Revert to this*, which puts the skill back as that version had it in one
  commit. A skill can be **deprecated**, pointing to the skill to use instead: it still works, and
  is listed after the others. A skill can be **archived**: it never loads, and *Restore* brings it
  back. A skill no chat ran for 60 days suggests deprecating it. A skill can name its owners, for
  people to know. The tab lists the skills a skill names, marking one that does not exist, and the
  skills that name it, and a run that started inside another skill's run says so. `purlis skill
  history | revert | deprecate | archive | owners | uses` do the same from a terminal.
- **A skill's grant file that is a link is no longer followed** (#2782). purlis reads what it keeps
  beside a skill's folder (its grant, and now its state and owners) only from a plain file: a link
  in its place grants the skill nothing beyond the project's root, and a change to it is refused,
  so a link can no longer widen where a skill reaches.
- **A skill's tab totals its runs** (Skill studio preview, #2783): runs, tool calls, errors,
  refusals, asks, tokens and time, by the program each chat ran on, by persona and by
  workspace. Each run says what it ran on, and a run that a curation action's prompt started
  says so, whichever way the chat's program started the skill.

### Fixed

- **A skill source of more than 50 000 files is always refused whole** (Skill studio preview).
  On a busy machine its listing could be read cut short and refused for the wrong reason, and a
  listing whose read failed was taken as complete (#3115).
