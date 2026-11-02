### Added

- **Try the Skill studio while it is built.** Settings › You › Previews has a _Skill studio_
  switch for this machine. With it on, every chat gets the project's own skills (a folder in
  `skills/` at the project's root, for chats at the root), on Claude Code, Codex and opencode,
  and the palette's _Open Skill studio_ lists every skill a chat gets and why any other is left
  out (#2765). Each time a chat uses a skill, its tab shows the skill and the tool calls made
  so far until the turn ends, and the skill's own tab lists every run with what happened inside
  it: tool calls, errors, refusals, asks, tokens and time (#2766). `purlis skill runs` prints
  them. A chat can also list, find and load its own skills and see their recent runs, so a
  chat program that has no skills of its own still gets yours. Loading a skill counts as a run,
  and the run says the chat loaded it (#2776).
  _Use skill…_, in the palette, on each chat tab's menu and on the Skills chip of a chat's
  pane, lists exactly that chat's skills, asks for the arguments a skill declares, and types the
  skill into the chat for you to send, in the way that chat's tool expects it, on Claude Code,
  Codex and opencode alike. The chip and the tab's menu show how many skills and how many
  characters of their descriptions the chat lists, and warn at four fifths of purlis's budget;
  a manual-only skill costs nothing where the chat leaves it out, as Claude Code does (#2775).
- **Extensions can add to the Skill studio, and run confined when they do** (Skill studio
  preview, #2777). An extension can offer templates for new skills, add its own findings to a
  skill's live lint, add panels to a skill's tab, and offer skills from a source that is not git,
  whose files purlis reviews as it reviews a git source's. Such an extension runs in the same
  sandbox a chat runs in: it reads only its own folder and what purlis hands it, writes only its
  own folder, reaches only the hosts its approval names, and never a host that serves an AI model.
  Its approval says so, and marks its source, lint and template as experimental. Where purlis cannot hold it in
  a sandbox, as on Windows today, it loads none of that extension. A skill's tab draws its Runs
  panel the way it draws an extension's panel.
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
  opens it. `purlis skill list` and `purlis skill show` list them too, each program's under its
  own heading, with whose each is and whether a chat gets it (#3008).
- **A skill can only narrow a chat** (Skill studio preview, #2771). A skill that asks to
  pre-approve tools or to add commands to a chat's events is handed to the chat without them,
  and says so; one that takes tools away keeps that. Each skill in the Skill studio shows, for
  Claude Code, Codex and opencode, whether everything it asks for applies there, and what is left
  out, carried over or not applied. A skill of a program's own or a repo's that would widen a
  chat is marked. A manual-only skill now stays out of the chat's choice, and costs nothing in
  its budget, on all three. Its badge says it runs only when asked for, and so does a Codex
  skill, its own or a repo's, that Codex's own settings keep out of its choice (#3050). The
  editor's live lint also warns of a description too short to say when to use the skill, a
  skill it names that does not exist, and a program the skill runs that this machine does not
  have. A chat that starts without a skill it would have loaded, because the skill is broken,
  says which and why on its tab, with a way to the Skill studio.
- **Move your project's skills into the Skill studio in one step** (Skill studio preview,
  #2784). `purlis doctor --fix project-skills` moves every skill in the project's
  `.claude/skills` into its `skills/`, shared with every workspace as before. It rewrites the old
  `charter:<skill>` names to `purlis:<skill>`, but never inside quoted code or an image name. It
  all goes in one commit that `git revert` undoes. It checks first that every chat keeps its
  skills, and changes nothing if one would lose one or if two skills or folders differ only in
  case, which a disk that ignores case reads as one. From then on every chat in the project gets
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
- **A skill that changes as it is moved is put back** (#3145). Sharing, renaming, moving,
  deleting or restoring a skill checks, once it has moved, that it moved what was checked; one
  swapped for a link in that instant is put back and the move refused, and a file with a second
  name (a hard link) is never moved. Where a skill could not be put back after a failed move, the
  message says what moved and what stayed, instead of that nothing was moved.
- **A skill's tab totals its runs** (Skill studio preview, #2783): runs, tool calls, errors,
  refusals, asks, tokens and time, by the program each chat ran on, by persona and by
  workspace. Each run says what it ran on, and a run that a curation action's prompt started
  says so, whichever way the chat's program started the skill.
- **The Skill studio offers a way to start** (Skill studio preview, #2785). A project with no
  skill of its own shows three starts above the skills its chats already get: *Ask a chat to
  draft a skill*, *Browse sources* and *New skill*, from the blank template. The sources
  section offers well-known collections of skills (Anthropic's, OpenAI's curated ones, and
  Superpowers), each with why it is on the list. *Add…* fills in Add source for you to
  confirm. Nothing is installed until you review a skill from it. A project whose skills are still where only some chat programs read them is
  offered *Move them*, the same one-commit move as `purlis doctor --fix project-skills`. The
  commit reaches everyone in the project, and a teammate on an older purlis opens it read-only
  until they update.
- **Grade a skill's trigger tests** (Skill studio preview, #2779). A skill can carry trigger tests:
  queries it should start on, and near misses it should leave alone. _Grade…_ on a skill's tab
  first shows what a grade runs: how many chats, each test as the chat will read it (a character
  no reader would see marked), what the skill's last grade spent, and that the chats are paid for
  by the profile you pick, as any chat is. A grade stops once its chats have spent 200,000 tokens,
  or the cap you set in the plan for this machine, or once they cost that many tokens would at
  the dearest price, where Claude Code says what they cost, and says what it spent. Nothing runs until you
  press _Grade_. Each test then
  runs in a chat of its own, three times with the skill and three times without it, in a
  throwaway folder, only in the project's full sandbox, with none of your own Allows, no tools
  but purlis's that only read, and only on a Claude Code profile that keeps its permission
  prompts; the plan says why when a grade cannot run. A chat that asks you something is ended
  there and does not count. A grade of a skill kept on this machine stays on this machine. Those chats get no tab, and each ends as soon as
  the skill starts. The tab says how far the grade has got and offers _Stop_. The skill's
  _Grades_ panel shows each grade: the version it graded, what it ran on, the tests passed, each
  side's pass rate and the difference the skill made. Each version in its history shows its
  newest grade. Grades are kept in the project beside its skill runs, so the team sees them, and
  a run a grade started says so. `purlis skill grade` prints the same plan and the grades.

- **A large library stays reachable without flooding a chat** (Skill studio preview, #2778).
  Once a chat's skills are past purlis's skill budget, it is handed the skills you pinned,
  purlis's own and one index skill that names every other skill in a line, and it loads any of
  them by name when a request needs it. Below the budget nothing changes. Pin a skill for a
  workspace's chats or a persona's with `purlis skill pin <name> -w <workspace>` or
  `-p <persona>`, and the router never hides it. The Skill studio and `purlis skill list` say
  which skills are pinned and why any other is hidden, and _Use skill…_ marks the hidden ones,
  says how many are in the index skill, and still starts them for you.
- **Pin and unpin a skill in the Skill studio** (Skill studio preview, #3232). Each skill's row
  in the studio for a workspace's chats or a persona's has a _Pinned_ switch, one commit each
  way. A skill's tab lists every workspace and persona that pins it, each with _Unpin_. Where a
  chat's pinned skills alone are past the budget, its Skills chip and _Use skill…_ name the
  studio to unpin them in, such as _Skill studio · ops_.

### Fixed

- **A skill source of more than 50 000 files is always refused whole** (Skill studio preview).
  On a busy machine its listing could be read cut short and refused for the wrong reason, and a
  listing whose read failed was taken as complete (#3115).
- **A skills folder purlis can't read whole is listed once and not read** (Skill studio
  preview, #3133). This applies to a repo's skills folder, your own skills folder and an
  opencode plugin or worktree folder, when one holds more than 2000 names or can't be read.
  The studio and `purlis skill list` name the folder and say why, instead of opening and checking
  every entry. A chat in a repo whose skills folder purlis can't read whole doesn't get the
  project's skills, since the repo might have its own copy of any of them.
- **A skill whose SKILL.md purlis doesn't read is listed, with the reason** (Skill studio
  preview, #3133). In a repo, that is a SKILL.md that is a link, a hard link or larger than
  256 KiB. In your own skills folder, it is a link or larger than 256 KiB. A hard link is read
  there, as the Nix store, `cp -al` and `rsync --link-dest` leave them. Such a skill used to
  vanish from the list.
- **A chat is told which project skills its repo leaves out** (Skill studio preview, #3213).
  On Codex and opencode, a repo's own copy of a skill replaces the project's, and a repo whose
  skills folder purlis can't read whole leaves out all of the project's skills. The chat's tab
  now names each skill it starts without because of its repo, and why. The Skill studio and
  `purlis skill list` list them under each program.
- **A repo skill named like a built-in one is said, not hidden** (Skill studio preview, #3027).
  A clone's own `secrets` skill is loaded by Claude Code and Codex beside purlis's built-in
  `secrets`, and a chat may use the repo's. The Skill studio used to call the repo's copy
  hidden. It now marks the built-in skill with the clash, the repo's row warns that its name
  is a built-in one's, and a chat started in that repo says so on its tab.
