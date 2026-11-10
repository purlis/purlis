# Changelog

What each release of purlis, the desktop app, brought. About purlis shows the section for the
version you are running, and the same section is that version's GitHub release notes.

The app has its own version line, starting at 0.1.0. It is not the version of the Python
`purlis` it was rebuilt from. `purlis news` prints this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.1] - 2026-11-10

0.5.1 is the first release after 0.5.0. You can steer a chat from its diff: see what one turn
changed, step back through earlier turns, discard a hunk, undo a turn, and see check results on
the diff. A chat can run a program with a vault's keys once you allow it, and the commits a chat
makes can be signed, on GitHub and GitLab. When purlis itself fails, it drafts a report for you
to file from your Inbox, and asks once whether it may send reports. New clones go to a
workspace's `repos/` folder, Explorer shows and writes the workspace's own files, and you can
drop files there from Finder. Large projects open, save and search much faster. The Skill studio
acts on several skills at once, and secret fills and browser logins are held tighter.

**After updating on Linux:** the app's program is now `purlis-app`, so a script or shortcut of
your own that runs `charter-app` needs the new name (see *Changed*).

These are the highlights. What was added, deprecated and fixed for security is here in full,
and every other change by its headline. All 446 changes, in full, are in
[Every change in 0.5.1](https://github.com/purlis/purlis/blob/v0.5.1/docs/changelog-0.5.1.md).

### Added

- **A built-in skill, `purlis-remember`, for "remember this", "add a todo" and "what did we
  decide".** It holds the one rule for where a fact goes: a lasting decision or a coined term
  into the workspace's `workspace.md`, what the workspace's work taught into its memory, how a
  persona's role is done into that persona's memory, and what every persona needs into shared
  memory, never a secret. It recalls across every memory at once, closes todos, and asks you
  before a todo is opened as an issue on a forge. Smart close, tidy-memory, the persona and
  safe-remove skills and a chat's briefing now point to it instead of each saying the rule their
  own way.

- **A Check reporter turns a tool's output into SARIF.** An extension that declares the
  `check-reporter` capability, once you approve it, is handed a check's output and exit status by
  `purlis check report <branch> <output> --reporter <extension> --status <n>`, and the SARIF it
  answers is drawn on the branch's diff like any other check result. If what it answers is not
  SARIF, the review says so in a Notice naming the extension, and the results before stay. Extension
  protocol 6 carries the new request (#5410).

- **purlis notes when a built-in skill should have started and did not.** When you start a
  built-in skill yourself, with Use skill… or a slash command, right after a turn that did not
  start it, purlis records a miss in the project's skill log: the skill, the harness and its
  version, and the persona, never your prompt. A built-in skill's page in the Skill studio shows
  its misses beside its runs, by what it ran on. Nothing is uploaded (#5738).

- **The same crash is reported once per version, and a report made offline waits.** A panic
  already sent from this machine in the same purlis version is counted, not sent again, and the
  count goes with the next report that leaves. A report made while purlis's report service cannot
  be reached waits on this machine (at most 50, for 14 days) and goes, oldest first, once it can;
  Settings › You › Reports lists it as "Waiting to send" (#5693).

- A file's diff can show what the latest turn changed, beside what the branch changed against its base. purlis records the branch's state before and after each turn of a chat, uncommitted work included, and removes that record when the chat is removed. A chat whose program does not report its turns says so instead.

- A file's diff can step back through a chat's earlier turns: One turn opens on the latest, and a picker beside it lists every turn of that chat, newest first. A turn that changed no file says so.

- **A browser login a chat suggests says when its vault has nothing under a key it names.** The
  notice reads "Vault qa has nothing under password yet", so a misspelt key is seen before you
  allow it, and an Always line doesn't write it unseen. Allowed anyway, purlis asks you for the
  value when it fills the login. Vaults kept by 1Password aren't listed for this, since listing one can
  ask you to unlock it.

- **A panic is sent as a Crash report, and you can see exactly what left.** With Crash reports
  allowed, a panic in the app goes to purlis's private report service (Sentry, in the EU) with
  only where it panicked, its message, purlis's version and channel, and your operating system;
  paths and the names of your projects, workspaces and repos are taken out first. Settings › You
  › Reports lists every report sent from this machine for 90 days, and each opens the exact bytes
  that were sent. The report service is listed in the network log (#5689).

- **A persona can bring another workspace's skill.** Write `workspaces/<ws>:<name>` on its
  `skills:` line: its chats in every workspace get that workspace's skill. The persona's own copy
  still wins. The named copy beats one of the same name in the chat's own workspace. With the
  Skill studio flag on, the studio as the persona lists it as `workspace <ws>`. `persona lint`
  warns of a workspace or skill that is not there (SK-5a, #2933).

- **A skill router is told what each skill costs.** Extension protocol 5's route question gives
  each skill `chars`, what its whole line costs of the budget on the chat's harness, and
  `index`, what the index skill costs when the router hides any. A router on Claude Code no
  longer under-counts and falls back to the built-in router. A router of protocol 4 is asked as
  before (#4680).

- **A chat on a pool's runner lists the skills its runner armed it with.** Use skill… and the
  chat's Skills show the skills the runner handed the chat, by name and level, when it starts
  and after the window reattaches it. A runner that runs a purlis older than this one doesn't say
  them, and the chat says so instead of listing none. Behind the Skill studio's flag (#4218).

- **A chat on a pool's runner says the skills its persona names that the runner doesn't have.**
  A local runner, a Docker pool or a k8s pool checks the persona's skills against what the chat's
  harness has there, and the chat's tab shows the same Notice a chat on this machine does,
  naming the runner. A runner that runs a purlis older than this one doesn't check. Behind the
  Skill studio's flag (#2977).

- **A chat on a pool's runner has its turns too.** The runner records each turn of its chats where
  they work, and the desktop fetches them with the runner's code, so One turn shows what a runner
  chat's turn changed, as it ended there. Undo a turn on such a chat asks the runner, which puts
  the files back while the chat is idle, with the same refusals as here. Its turns go with the
  chat on both machines. A runner on an older purlis records none, and One turn says so (#5427).

- **purlis now notices when a skill names another skill bare, as in `Use the Skill tool with
  "grilling"`.** `purlis skill uses` lists it as a hint, the skill lint suggests writing
  `skills:grilling`, and installing the skill brings `grilling` along from the same source. A
  name counts only when it is quoted or in backticks, its line says "skill", and a skill of that
  name exists (#4510).

- **A skill source carries a skill's grade, and the install review shows it as the publisher's.**
  Publishing a skill also writes its grades to a file beside the skill's folder. Installing or
  updating from that source shows the grade under **Grade**. It says whose grade it is and that
  purlis hasn't checked it, before any figure, then whether it was measured on the version you
  are installing. A grade file purlis can't read is left out, and the review says why (#4346).

- **An error the window did not catch is sent as a Crash report, as a panic is.** With Crash
  reports allowed, an uncaught error or unhandled rejection in purlis's window goes to purlis's
  private report service with only its name, its message, the file and line it was thrown at,
  purlis's version and channel, and your operating system; paths and the names of your projects,
  workspaces and repos are taken out first. It is listed in Settings › You › Reports with the
  exact bytes sent. Without Crash reports allowed, nothing is sent (#5690).

- **A chat drafts the purlis bugs it meets.** The new built-in skill `purlis-report`, on Claude
  Code, Codex and opencode, tells a chat when something is purlis's to fix (a purlis command
  that fails, the window showing something false, you saying purlis is broken or slow, a chat
  working around purlis, a purlis skill that did not start) and what is not (your project, the
  harness itself), and has it draft through `purlis report` into your Inbox, where you read and
  file it. While you allow Agent drafts (Settings › You › Reports), each chat is told about it
  at its start.

- **Always send: report drafts can go without you reading them, only to purlis's private report
  service.** Press Always send on a Report draft in the Inbox. The first time, purlis asks, says
  where drafts will go and where to see and stop them. After that, a chat's draft skips the
  Inbox. It shows as "Sent a report" with Undo for a few seconds, then goes to purlis's report
  service, never to GitHub. Undo keeps it in the Inbox unsent. At most five go a day; after that,
  drafts wait in the Inbox for you again. Turn it off under Always send drafts in Settings › You ›
  Reports, a row shown while Agent drafts is allowed (#5695).

- **The right bar's icons say what they are for.** Hover or focus Asks, Updates or Notices and
  its tooltip asks its question: "Do I need to answer something?", "Did my work finish?", "Is
  something about my setup wrong or worth knowing?". The ✋ pulses once, the first time on this
  machine an ask arrives while Asks is closed, and stays still under reduced motion. When nothing
  waits, Asks says "Nothing waits on you · 2 chats working", and each part opens the Chats
  sidebar. A screen reader hears a new ask once, politely, and each icon's count is part of its
  name (ADR 0121).

- **Bring back a deleted skill whose name purlis holds now, under a new name.** On Deleted, one
  skill or a group brought back together offers each such skill a new name, filled in as an
  install fills one when a name is taken (`<source>-<name>`), and brings it back once you
  confirm it; in a group the rest come back as named, in the same change (#5457).

- **Bring back, move, export and delete now several skills from the command line** (#5492).
  `purlis skill restore` and `mv` take several names, `mv` and `export` take `--source <url>`,
  and `purlis skill delete-now` takes skills off the Deleted list now. Each makes its change as
  one step after printing its review and asking you at your terminal. Delete now asks even for
  one skill, since it can't be undone. A chat or a script with no terminal can't make these
  changes: the command writes nothing and puts the change in front of you as a proposed change.
  For a proposed export, you pick where it goes when you apply it.

- **Change several skills at once from the command line** (#5357). `purlis skill rm`, `off`,
  `on`, `pin` and `archive` take several names, or `--source <url>` for every skill installed from
  a source, and make the change as one commit after printing its review and asking you at your
  terminal. A chat can't make such a change, and neither can a script with no terminal: the
  command writes nothing and puts the change in front of you as a proposed change, which you
  apply or dismiss in the window.

- **A branch's check results on its diff.** `purlis check report <branch> <file.sarif>` hands purlis
  a tool's results in SARIF 2.1.0, from a script, an agent's hook or CI. "What changed" in a file then
  marks them in the gutter on the changed lines and lists each with its tool, severity and message,
  those outside the changed lines and in other files too. Each opens at its line, in your editor as
  well, and they are marked out of date once the branch's head moves (#5407).

- **purlis asks again once, after a crash, if you chose Not now.** The next crash purlis catches
  raises one Notice in the Inbox: *Turn on crash reports* turns them on, *Open Reports settings*
  opens Settings › You › Reports at Crash reports, and Dismiss means purlis does not ask again.
  It never appears a second time (#5700).

- A source's page has **Delete all…** and **Turn all off…** or **Turn all on…**: tick the skills, read one review, and it is one commit you can undo. **Deleted** groups them, with **Bring all back**, and each skill can still come back on its own.

- **Remove source…** can also delete the skills installed from it, through the same review.

- The Library has a **Source** filter, and a tag on each installed skill's row sets it.

- **Discard one hunk of an agent's change.** Each change in a file's *Show what changed* has a
  *Discard*, which puts it back as the branch's base has it and takes it out of the diff at once;
  a binary file's change goes whole. It warns when a chat is working in the folder, offers Reload
  or Overwrite when the file changed on disk, and is recorded in the chat's history as a Human edit
  of its file and lines, never its contents (#5406).

- **Dismiss all, with Undo, on Mine, Project and Notices.** Each list's Dismiss all does each
  line's own Dismiss to every line it lists: an update goes for its day, a Notice until its cause
  changes and comes back when it does, and a teammate's change to the project is recorded as
  seen until they change it again. The lines go at once and an Undo stands for a few seconds,
  after a single Dismiss too; only when it goes is anything kept. What you dismiss is kept on
  this machine through a relaunch and never written into the plane (ADR 0119, #5650).

- **`purlis doctor` lists the repos still at a workspace's root, and moves them into `repos/`
  when you ask.** Repos cloned before new clones went to `repos/` live beside `workspace.md` and keep working there with
  nothing done. The doctor's *repos outside repos/* row names them, and its fix, *Move into
  repos/* (`purlis doctor --fix move-into-repos`, or the row's Fix button), moves them one
  workspace at a time. It is all or nothing for each workspace: a workspace with a chat open in
  it is left as it is, and so is one with a submodule checked out in a branch folder,
  which a move would break. A failure partway puts everything back. Every branch folder cut from
  a moved repo is repaired, so it keeps working. Before it moves anything, the window names the
  chats started in a repo itself, which their harness can no longer resume; their session
  records still open. The doctor also warns about a folder beside the repo of its name in
  `repos/` (the one in `repos/` is the repo), and about an old repo named `repos`, which holds
  new repos back until you rename it.

- **Drop files from Finder onto a workspace's files to import them.** With the workspace picked,
  files dragged from your file manager onto a folder of your own in Explorer are copied into it,
  and onto no row into the workspace's top, as Import files… copies them: a name already taken
  gets a ` (2)` suffix. A drop on what purlis keeps, a repo or `repos/` says why it imports
  nothing (#5378).

- **An extension's lint speaks in the install review.** With the Skill studio on, each extension
  that lints skills is asked about every skill a source's install or update review shows, in the
  window and at `purlis skill install` and `update`, and what it finds is a lint line under its
  name, beside purlis's own (#3090).

- **Install from an extension's skill source on the command line, and update its skills together**
  (#5760, behind the `skill-studio` flag). `purlis skill install --extension <id>` lists, reviews
  and installs from the source an extension adds. It asks at your own terminal each time and has
  no `--yes`. A chat is refused, as for a git source. Review updates now takes an extension's
  skills beside git ones, each copied from what its own review read, in one commit with one Undo.
  An update that adds a script or other new capability still starts unticked. An extension's
  source page marks what each skill's files do, as a git source's does.

- **Sources lists each extension that offers skills beside your git sources** (#3092, behind the
  `skill-studio` flag). You browse it, review an install and install from it on the same pages,
  at the revision the extension names. An update to one of its skills is reviewed and accepted
  from that skill's row.

- **A fresh clone opens quiet.** The first time a project opens on a machine that has never seen
  it, what is already there counts as seen: a teammate's past change to the project's hosts or
  Internet access presets is not told as new, an update from before that open is listed under
  Updates' Project, read, and the doctor's findings already there are put away until they
  change, so the Notices view's dot stays quiet. A finding that appears later is new. The open
  asks the person may answer stay in Asks, and so do a teammate's dispatch grants and browser
  sites, which only the person's answer puts in force. A project this machine has opened
  before is never baselined, so updating purlis keeps every unread line. The record is kept on
  this machine and never written into the plane (ADR 0119, #5648).

- **Grade several skills at once.** In the Library's Select mode, Grade shows what the grades
  should spend from each skill's last grade, says which have nothing to go on and which have no
  tests, and runs three at a time under a spending limit you set. Reaching it stops them cleanly,
  keeping every grade that ended (#5356).

- **An agent hears what you changed at its next turn.** When you discard a hunk of a chat's
  change, that chat's next turn starts with one line of context naming the files you changed
  since its last turn, paths only and never their contents, so it reads them again before it
  builds on code that is gone. Changes from several sittings come in that one line, it is never
  sent as a prompt for you, and it is not repeated at the turn after (#5409).

- **The review says when a chat isn't told of your change.** When you discard a hunk or undo a
  turn, a chat working in that folder on a harness purlis has no prompt hook into, such as one
  your project declares, is named under the diff: its agent is not told at its next turn, so you
  can tell it in its chat. Claude Code, Codex and opencode chats are still told (#5477).

- **Ignoring a chat's ask in Asks has an Undo.** The ask goes at once and an Undo stands for a
  few seconds, as it does after Dismiss; the chat is ignored until it asks again only when the
  Undo goes (ADR 0119, #5638).

- **A skill from an extension's skill source installs and updates like one from git** (#3089,
  behind the `skill-studio` flag). The install copies the files you reviewed, never asking the
  extension again, and pins the copy to the extension's revision in `skills.lock`. A file that
  holds such a pin is version 2, which an older purlis refuses whole. `purlis skill update`
  lists and updates these skills too. An answer naming two paths that differ only by case is
  refused before the review.

- **Select several skills in the Library and act on them at once.** The Skill studio's Library has
  **Select**: tick rows (Shift-click or Shift+↑ ↓ for a range, ⌘A for every skill the search and
  filters match), and the page beside the list holds the count, every action with how many it
  reaches, and the names, including those the list does not show. Turn off or on, pin or unpin,
  archive or unarchive, move to another level, export or delete them, each through one review
  and one commit with one Undo. Built-in skills can be selected and every action leaves them as
  they are, and says so. The Deleted page has Select too, with **Bring back** and **Delete now**,
  which asks first as it cannot be undone (#5332).

- **A LIVE workspace's files say where they are kept.** With the workspace picked, each file and
  folder in Explorer is marked *This machine only*, or *Saved with the project* when the
  workspace's LIVE allowlist holds it. **Save with the project**, in the menu of one of your own
  files or folders at the top of the workspace's folder, adds exactly that entry to the
  allowlist, so your next save shares it. Nothing is added any other way, since a script may hold
  a token, and a workspace that isn't LIVE shows no marks.

- **Make room when a chat is over its skill budget.** The Skill studio's **Routing** page, and the
  **Install all…** review when an install would go over, offer **Make room…**: the Library's
  Select, with the skills least used in the last 30 days first, each saying its runs, and nothing
  ticked. Turn off or archive what goes, or pin what stays (#5355).

- **A chat can list and reopen done todos.** Its `todo_list` tool takes `done: true` to list the
  workspace's done todos, as `purlis ws todo --done` does, and the new `todo_reopen` tool opens
  one again by its slug, as `purlis ws todo reopen` does, writing nothing to memory (#3416).

- **New marks what you have not seen.** An ask, a Notice or an update you have not seen yet has
  an accent bar at its start, and the Asks, Updates or Notices icon a small "new" pip. It is seen
  once its row has been in sight for about a second, or once its chat is on screen in a focused
  window, so an ask from the chat you are looking at is never New, though it is still counted.
  What you have seen is kept on this machine and survives a relaunch (ADR 0121).

- **An older runner kept for work purlis can't bring back can be let go in Devices.** Its row
  offers Let it go…, which names in plain words what is lost and asks first; only that runner is
  removed, and its pool and the pool's runner now stay as they are.

- **One notice for every lane refused for unsigned commits.** However many lanes a forge
  refused, the Inbox lists them in one notice ("3 lanes can't merge: <repo> only accepts signed
  commits"), and the needs-you hand in the title bar counts it once. **Set up signing** opens the
  same dialog, which ends in **Sign and retry all 3**; with signing on, the notice offers that at
  once. A lane refused again stays listed with its reason; once all went, the notice says "3
  lanes signed and sent ✓" (#5633).

- **A pool's row names the project its older runner waits on.** After an update, the old runner stays until each project its chats ran for brings their work back, which only an open project does. The row now names each such project, says when a closed one was last opened, and says that opening it, or removing the pool, lets the runner go.

- `purlis recall` answers from a project search index the session host keeps, so it stays fast on a large project. It finds the same memories as before, now ranked by relevance, and says `refreshing` when the index may be behind the files. Deleting the index is safe: recall reads the files until the host has built it again. A secret-shaped span in a memory never reaches the index.

- **A project that fell short at launch says so on its row, with one Retry.** A project that
  failed to open shows its error on its row in the projects sidebar; one still loading after 15 s
  and one with "N of M chats back" offer Retry too. Retry reopens that project alone and brings
  its missing chats back, and it is the same action as a waiting task's row in the sidebar, which
  now starts every chat its project is short of (#5324).

- **A project can keep its report drafts from going unread.** Settings › Project › General gains
  "Members may let agents send reports without asking", on by default and kept in the project's
  settings, which your team sees. Turned off, no draft written in that project goes to purlis's
  report service unread, whatever a member's own Always send says: it waits in the Inbox, and
  members can still read and file it. Settings › You › Reports shows Always send read-only there,
  badged Project, with "Your project turns this off" (#5696).

- Publishing a skill to a source you cannot push to now goes through your own fork on GitHub or GitLab. The review names the fork, says who can read it and whether it will be made. The fork is made only when you press the button, and it stays afterwards. A fork you already have is used again.

- **A built-in skill for one change across several repos.** When you ask a purlis chat to keep
  work that spans several repos together, such as an API bump and the repos that use it, the new
  `purlis-change` skill guides it through `purlis change`: create the change, add each repo and
  what must land before it, push every branch with linked pull requests, land them one at a time
  in order, and roll the whole change back. It says that landing merges code and is yours to ask
  for, and that a pull request merged in the browser holds back the repos waiting on it.

- **purlis drafts a report of its own failures in a chat.** With Agent drafts allowed, a
  `purlis` command that fails inside purlis (it now exits 70 for an internal failure, never for
  a refusal or a usage error), a purlis tool that failed inside purlis, and a sandbox block of
  purlis's own operation each become a Report draft in the Inbox, with the call, its exit and
  its scrubbed error. The chat is told once, on its next turn, and asked to add what it was
  doing with `purlis report bug --adds <draft>` (#5697).

- **A built-in skill for permissions: `purlis-permissions`.** Say "stop asking me about this
  command", "why was this blocked" or "allow this host", and your chat now explains what stopped
  the command: a harness prompt, purlis's sandbox or a purlis guard. It then points you to where
  purlis reads the change: a `purlis guard ask` or `allow` rule, the sandbox Notice, or your
  sandbox settings. It never sends you to edit your harness's own settings file or to lower the
  sandbox, because purlis's settings outrank those. Requests about your harness's own settings,
  such as an environment variable, a theme or the model, stay with your harness. The skill has
  trigger tests at the strict bar.

- **A built-in skill for a workspace's life.** Ask a chat to make, fork, restore, rename, pick or
  share a workspace, or to save and push the plane, and it follows `purlis-workspace`: the right
  `purlis workspace` command, and the rule that goes with it. It asks before making a workspace
  live or running `purlis save`, because both publish, and it says up front that a fork does not
  copy your repos, that a rename waits until no chat runs in the workspace, and what to run in
  your own terminal when a sandboxed chat is refused.

- **Quiet.** Notifications wait while it is Quiet, and the ✋ still counts. It is Quiet in your
  quiet hours, while "Quiet now" is on (a button at the top of Asks, or the palette), and while you
  type in a chat, until about half a minute after your last key. When Quiet ends, one row at the
  top of Asks sums up what came in, "While you were quiet: 3 asks, 2 finished, 1 failed", each
  part a link, and it goes once you have seen all of it. If you are away from the window, one
  notification names the chats still waiting (#6017).

- **purlis asks once whether it may send reports.** On the first launch, and once on the first
  launch after this update, a dialog asks about Crash reports (Send or Don't send) and Agent
  drafts (Allow or Don't allow), with nothing chosen for you; Not now leaves both off. *See
  exactly what's sent…* shows a sample of each before you answer. Both answers live in
  Settings › You › Reports, which the Command palette and the Settings filter also find by
  "crash", "telemetry" and "error reporting". Nothing is drafted or sent for a kind of report you
  have not said yes to (#5688).

- **A chat's report waits in your Inbox for you to file.** With Agent drafts allowed in
  Settings › You › Reports, `purlis report bug` or `purlis report feature` run in a chat leaves a
  Report draft card in the Inbox: the chat it came from, the call that failed and the versions.
  Read shows the whole draft, marked as an agent's and unreviewed, and you can change its words.
  File sends it to purlis/purlis as you when you are signed in to GitHub, after a check for
  issues that may already cover it (Add to one instead, or File as new), and to purlis's report
  service when you are not. Dismiss lets it go unsent. A filed draft leaves a "Sent a report"
  update, and the sent list now lists reports filed on GitHub too (#5694).

- When purlis reopens a chat after a quit or an update, the wake line now says what each piece
  of the chat's own work last put out. For a background shell or a monitor, it quotes the last
  20 lines of output and says where the full output is, or says the output is gone. For a
  subagent, it quotes the last message, and for a workflow, the last thing it reported. A
  wake-up that came due while purlis was down is listed as "was due at <time>, missed". Crons
  and wake-ups that are still ahead are listed as ones Claude Code sets again itself, so the
  chat does not set them twice.

- **Resume a stopped grade instead of grading it again.** A grade that stopped at its token cap,
  at the program's usage limit, with no network or by your Stop now says why, and its scorecard
  offers Resume beside Grade again: only the cases it did not finish run, under a new cap you
  confirm first, and the scorecard shows both runs together. Resume is offered only while the
  skill, its tests, the program, the model and the grade's settings are as they were, waits for a
  usage limit to reset, and never runs by itself; Needs you says when it can. A draft's grade
  resumes too, and Select resumes several stopped grades under one limit (#5718).

- **Review updates for several skills at once**, from the Library's **Select** mode and from a source's page (**Review updates…**). One page lists every update, those that add something new first. An update that only edits a skill's text starts ticked. One that newly adds a script, a hook, a tool, network access, a write outside the workspace, hidden characters or a file that isn't text starts unticked, and you can tick it only after opening what it changes. Accepting is one commit with one Undo, and chats keep the version they have until you accept.

- **Allow one program with a vault's keys.** Where a chat's `secret exec` names a program its persona doesn't declare, a notice shows the program's full path, the keys, the hosts, the folder and the command as plain text. Allow once runs that exact command in the chat's own sandbox, with no input from the chat, and Don't allow is not asked again about that program and vault. A vault the persona can't otherwise use opens for that alone, with nothing kept. Programs that run what they're told, and `purlis` itself, stay refused (ADR 0112).

- **Always allow a program.** For a program purlis has reviewed, `curl` so far, the notice shows the exact tool line for the project's file and writes exactly that, and the waiting command runs with no chat restart. Where the persona was fully allowed on this machine, the Allow moves for that line alone; where anything else is pending, the usual Notice stays up and nothing else is approved. A teammate's machine runs nothing until its own Allow, and a write that fails says why and gives no grant.

- **Allow a program for a whole chat.** For a program purlis has reviewed, `curl` so far, Allow for this chat lets it run with those keys and any arguments until the chat ends, and a key added asks again. Any other program gets Allow once on the exact command. Run grants appear in the Allowed list with a Remove that applies from the next run. Each one given, taken back or not allowed is audited by program, keys, vault, scope and chat, never a value.

- **A policy lock on Run grants.** `sandbox.run-grants`, allowed by default, turns them off. Under it, or under the `persona-tools` lock, no Allow is offered and the chat hears why and to declare the program as a tool, and a key the command misses can still be filled. A chat on a runner gets no Run grant and hears the same. A refusal under policy is audited.

- **The Signed commits card manages your key.** Once on, it shows a row for each GitHub and
  GitLab account, with "Verified ✓" or **Add to this account**. It lists your other devices'
  purlis signing keys, each with **Remove**, for a lost or wiped Mac. **Replace key** makes a new
  key and puts it in place of the old one on every account. **Remove signing** deletes the key
  from this Mac and from every account it was added to, after one confirmation that names them;
  commits already signed stay signed (#5632).

- **Set up signing fixes a commit email the forge won't verify.** GitHub and GitLab show a
  signed commit as Verified only when its email is one of the account's verified addresses. When
  it isn't, the Check step says so in a sentence and offers those addresses (on GitHub, the
  private noreply one too) to sign chat commits as, then checks again. With none verified, it
  links to the forge's email settings (#5631).

- **Signed commits on GitLab**, gitlab.com and your company's own server alike. Setup adds the
  key to your GitLab account as a signing key only, so it can't be used to log in, with the
  sign-in purlis already has: no extra permission to give. GitLab can't check a commit purlis
  hasn't pushed, so "Verified on GitLab ✓" shows once a chat has pushed its first signed commit
  there and GitLab says so. A GitLab server too old to verify signed commits says so, and isn't
  offered setup. The card shows a row for each GitHub and GitLab account (#5629).

- **Signed commits, set up from the window.** A **Signed commits** card in Settings › You ›
  Accounts sets up the key purlis signs your chats' commits with, in one dialog of three steps:
  - **Make your key.** purlis tells you before macOS asks for the Keychain.
  - **Add it to GitHub.** purlis adds it through your account, asking GitHub's permission once,
    or you add it yourself and the window carries on once GitHub has it.
  - **Check.** GitHub itself confirms a signed test as Verified.

  The card then shows "Verified on GitHub ✓" with the email your commits use. The key stays in
  this Mac's Keychain, and no chat can reach it (#5628).

- **A refusal of unsigned commits says so, where it happened.** When GitHub refuses a chat's
  push because the repo only accepts signed commits, the lane's row in Tasks and the chat show
  "GitHub only accepts signed commits here" with **Set up signing**. Once signing is set up,
  **Sign and retry** signs the branch's own commits, never what's already pushed, and sends it
  again: "Signed and sent ✓". The chat is told plainly it can't set signing up itself (#5630).

- **An org's project controls where its skills come from.** A `[skill_policy]` table in
  `charter.toml` keeps an allow and a block list of skill sources and single skills, and a
  minimum age for a commit (7 days by default, with exact commits exempted by a reviewed
  change). The Skill studio's review and `purlis skill install | update` say why a skill is
  refused, and every install, update and removal is recorded in the event log the audit is
  written from (SK-11, #2774).

- **Preview a chat started in a clone in the Skill studio.** Where a workspace's clones hold repo
  skills, the studio's Whose chats picker also chooses where the chat starts: at the workspace's
  root or in one of those clones, whose repo skills count against the chat's skill budget too.
  The Library's hidden marks, a skill's page, Harness skills and Routing all follow it, and a
  skill's page lists whether it is listed or hidden in each place (#4227).

- **Snooze all on Asks.** After one confirmation inside the view, it puts away every listed
  chat's asks, answering nothing and with no timer, and the ✋ stops counting them. A chat comes
  back, with all its asks, as soon as it asks something new; until then the Chats view still
  says it waits, and Asks says how many are snoozed, with Show them now. An Undo stands for a
  few seconds after. What was snoozed is kept on this machine, per project, through a relaunch,
  and never written into the plane (ADR 0119, #5647).

- **Stricter terminals**, a project's sandbox setting in Settings › Sandbox, off by default
  (`own-terminal-only = true` under `[sandbox]`). On a Mac it keeps a sandboxed Codex or opencode
  chat to the terminal it runs in, so it can't read or type into your other terminal tabs or
  another chat's terminal. While it's on, a chat can't make a terminal of its own, so `script`,
  `expect`, `tmux` and Codex commands that run in a terminal fail; the setting says so.

- A working chat reads Stuck? when nothing new has come from it for 15 minutes, the same command has failed 3 times in a row, or one turn has run longer than an hour. It is listed in Updates while it does, and "It's fine" clears it until the chat changes state. Each threshold is a setting in Settings › Project › Dispatch (#6014).

- **The commits a chat makes are signed, so a repository that requires signed commits takes
  them.** `purlis signing setup` makes a key (or takes yours), keeps it in this machine's
  keychain, adds it to your GitHub account as a signing key and checks that GitHub has it, in
  three plain steps. From then on the app signs each commit it makes for a sandboxed chat, and
  the commits `purlis worktree update` makes by a rebase or a merge, and GitHub shows them as
  Verified. The chat never sees the key. With no key, the commit is still made, and the chat is
  told it is unsigned and how to fix that. `purlis signing check <owner/repo> <commit>` asks
  GitHub how a pushed commit shows (#5480).

- **The strip's `+` has a list.** Pressing `+` still opens an empty tab. Rest the pointer on it, or
  press Down on it, to choose what else opens in a new tab: New chat, New shell, a browser,
  Settings and the other views, each with its key. A long list has a filter (#5292).

- **Settings › Theme shows the component parts a theme sets.** Each card counts the part keys its
  theme sets. A new Parts row names the families the chosen theme sets, each with the parts
  manifest's description of it, and says what it draws from a theme it extends. Under Show each
  part is every family and part purlis has, with the states each draws, the properties each part
  takes and the version it came in. Under a part are the keys the theme draws, each with its
  value and what sets it: the theme, a theme it extends, or one of its options. A filter finds a
  part by its name, a description, a property, a state, a key or a value (#2947).

- **A theme's options can change component parts.** A toggle or a choice in `theme.json` can
  carry `parts` beside its `tokens`, so a theme can offer, say, round buttons as a switch in
  Settings › Theme. The theme check holds every combination of options to the same bounds and
  floors as the theme's own parts (#2946).

- **Put your pinned workspaces in your own order.** Drag a workspace's chip along the title bar,
  or use Move left and Move right on its menu from the keyboard. The order is kept per project
  across launches, ⌘1–⌘9 (Ctrl+1–9 off a Mac) follow it, and no chip changes size while you drag
  (#3128).

- **See which workspace has a chat working from the title bar.** While a chat in a workspace is in
  its turn, that workspace's chip wears the working ring a chat wears on its tab, on its lower
  corner, and a screen reader hears "1 chat working" on the chip. The mark is a shape, never a
  colour alone, it stays still, and it moves no chip as it comes and goes (#3127).

- **Undo a turn.** On the latest turn not yet undone, a file's diff offers *Undo turn N…*, which
  asks once, listing every file it will change, and puts each back as it was before the turn:
  files the turn added, deleted or renamed included, and nothing committed. It is refused while a
  chat is working in the folder, naming the chat and its state, and when a file the turn changed
  has changed since, naming the file. After an undo the turn before it is offered. The agent's
  next turn is told that a turn was undone, by the files' paths alone (#5411).

- **Discard on a turn's diff.** *Discard* is back on one turn's diff, and puts the hunk back as it
  was at that turn's start, not the base. A file you changed since the turn ended is drawn from the
  turn's start to the file as it is now, so each hunk left can be discarded too (#5411).

- **Your own workspace files can be written from Explorer.** With the workspace picked, a folder
  of yours offers **New file…**, **New folder…** and **Import files…**, and any file or folder of
  yours offers **Rename…** and **Delete…**. The view's head has New file, New folder and Import
  files for the top of the workspace's folder. Delete sends to the system's Trash after one
  confirmation. Where there is no Trash it is refused, and nothing is deleted for good. An import
  copies the files you pick, and a name already taken gets a ` (2)` suffix. A rename or delete of
  a file a chat is writing says so first. Repos, branch folders and the files purlis keeps stay
  read-only, and no write reaches outside the workspace's folder (#5376).

### Changed

- What purlis tells a chat says a chat is in a pane, not a tab.
- Dragging over a folded pane says why it takes no drop.
- A large project opens without waiting for its watch on macOS.
- A browser login a chat suggests marks the part of each host its owner registered, so a known name borrowed on its left, or a swapped letter, stands out.
- A browser login a chat suggests on a page other than the site's front page says so.
- A chat on a runner starts only once its profile's harness settings file can be read.
- A browser login notice from a chat on a pool's runner goes after 15 minutes with no answer, as one from a chat on this machine does, and the chat is told nobody answered.
- `purlis worktree update` standing in a workspace's clone, for a sandboxed chat, no longer rebases there in the app.
- Sending a task back says it goes back to its row.
- Sending a task back says it stays in the Chats list.
- The window says "shell", not "shell tab".
- A sign-in lets the chat act on its provider's site only in the sign-in's own window.
- A browser holding a login takes a secret only for the login's own sites.
- A sign-in that asks for a second secret on its provider's site finishes by itself.
- A file a blank or embedded frame made is said to come from the site around it.
- A skill you have twice is listed once.
- A split tab keeps its first chat's name, shows the pane that needs you, and pictures its layout.
- A tab's name has room.
- The title bar's crumb and a switcher row take a control's corner.
- A `purlis` command that waits on you says where to answer.
- A chat's browser act lands only where its element is drawn.
- An older runner of a pool stays while it holds work purlis can't bring back on its own.
- An older runner purlis keeps unchecked now says so in plain words.
- An ask is answered in its row, behind an Answer link, and a result opens in place.
- Asks and Updates are walked as lists of rows. ↑ and ↓ now move from one ask (or update) to the next, and ← and → move through an ask's own links, answers and reply box, where ↓ used to step through every button of an ask before the next.
- Asks heads the right sidebar, where the Inbox was.
- A chat in a branch folder leaves the clone's git to the app.
- A rebase stopped in a clone goes on or back by the chat's own git.
- The app keeps its own record of an update in progress, under the project's state, not in a git directory.
- `purlis worktree push` asks the remote what it has, and scans what the push sends against that, not against the clone's remote-tracking branches.
- Chats' browsers run Chrome for Testing 157.0.8103.38, up from 155.0.8059.79.
- purlis's built-in skills, a chat's briefing and purlis's tool descriptions use one vocabulary.
- opencode chats list your plane's skills beside purlis's own. purlis's built-in skills' descriptions are shorter, together at most 5,000 characters, and opencode's default skill budget is 8,000 characters, so the built-ins no longer fill it alone (spec #5714).
- Codex chats list your plane's skills beside purlis's own.
- A chat starts faster on a project with many personas.
- A new chat's briefing reads the project index the session host keeps, so a chat starts quickly on a large project.
- A chat says one of eight states, with one icon and one colour each, the same on its row in the Chats list, its tab, its pane's head and in Asks: Working, Waiting on you, Waiting on a chat, Stuck?, Done, Failed, Idle and Can't tell.
- The Chats list works like every other tree in the window (#3198).
- A large project's saving status costs a fraction of the git work it did.
- A Claude Code chat on Linux under Claude Code's own sandbox hands a vault's value to a command by file.
- purlis runs Claude Code chats on Linux inside its own sandbox, on a profile for sandboxed chats only.
- Claude Code keeps the descriptions of your own skills and plugins.
- More of purlis's own failures exit 70 and are drafted as a Report.
- A clone in a workspace's `repos/` folder is seen wherever a clone at its root is.
- A wide workspace's Explorer no longer reads every repo's status to count its changes.
- A decision is answered on its row in Asks.
- Finding duplicate memories stays quick on a large store.
- A project that has run thousands of tasks no longer reads every past dispatch to draw the sidebar's task status or each turn's list of open tasks.
- Every chat on a runner starts inside purlis's sandbox.
- A sign-in in a shell on a pool's runner lets a drain carry it again, as the driven sign-in does, and a sign-out ends the watch of that sign-in.
- Every browser login starts clean on its own site.
- With the workspace picked, Explorer shows the workspace's folder.
- A skill from an extension's template waits in Drafts.
- Five built-in skills say when they are for, and which skill to use instead.
- Four built-in skills have names that say what they do.
- Get project's clone progress is the window's progress bar.
- Asks, Updates and Notices list their messages as rows, not boxed cards.
- Keep blocked on a browser login or a secret fill a chat suggests now holds for every chat of that persona until purlis restarts, and after two Keep blocked its chats stop asking.
- Keep blocked on a vault the chat's persona holds from the Allowed list links there: the Notice that says Keep blocked does not stop the chat's runs, in the Inbox and on the refused vault's own Notice, has Open the Allowed list beside Dismiss, which opens Settings › Sandbox › Network › Allowed.
- The hold on a workspace made during a sandboxed Claude Code chat is measured on Linux.
- A project of a thousand workspaces scrolls as smoothly as a small one.
- `memory_search` with no query shows the newest 20 memories, not all of them.
- A narrow pane head drops a chat's skill run and its browsers after the state's word, so a pane under 450px keeps the chat's name and mark (#5614).
- New clones go to a workspace's `repos/` folder.
- Nothing pops up by itself any more.
- The status line no longer repeats counts the right bar shows.
- The Notices dot is red or amber.
- One turn says why it has nothing to show for a chat on an older runner.
- The Notices centre's cards read like the Inbox's.
- Notices is a view of its own, first of the right side's Lower icons.
- A branch purlis already fetched from an older runner counts as brought back, though it was squashed or rebased here since, deleted, or its workspace renamed.
- One grip to move a pane or a chat.
- A chat's mark is now the same on its tab, its row in the list, its pane's head and a folded pane's rail.
- A task that failed shows a cross on its chat's mark until that chat has been on screen and focused, and the mark then goes back to the chat's own state.
- A run asks once for everything it needs.
- A secret's notice in the Inbox is one card that says who says what.
- One notification per chat.
- A large project opens and idles without the file watcher's cost.
- Open in terminal on a pool's profile now types `purlis profile sign-in <name> --pool <pool> --in-terminal`.
- opencode's own sign-in on a runner is carried to the runner that replaces it, as Claude Code's and Codex's are, when the runner it came from kept its opencode chats out of that folder.
- Opening a task beside names the chat it opens beside.
- A skill that runs its own scripts can pass its outcome evals.
- The pane foot's skills warning says what a press does in its row's words, as the foot's other parts do (`Skills of …`).
- The theme check holds the pane foot's warning number on the terminal.
- A pane's Split right, Split down and Close are always visible in its head, not only under the pointer.
- A chat pane's notices are compact rows under its head: one line each, with its icon, who asks and what about, its age where known, its own buttons and a × that hides it from this pane.
- A project's slow panels load on their own.
- Panes make room while you drag.
- A theme that fills a panel, a popover or a dialog is held to everything drawn on it.
- A persona's own sandbox hosts are written one per line, sorted.
- The project's settings file lists its allowed hosts and browser sites one per line, in order, so two machines that each add one change different lines (#5824).
- The save badge says when the project is stale.
- A project's search index builds about six times faster.
- The build provenance of a dev build, of the runner image and of the cluster operator image now names the commit each was built from.
- `purlis signing` no longer needs GitHub's `gh`.
- Quitting and Restart to update name the work of its own a chat would lose.
- The project rail and the activity bars wrap at their ends, as the tab strip does: Down on the last project or view goes to the first, and Up on the first to the last.
- Rename says what it renames.
- Rename chat… is in every chat's menu, by that name: a pane's ⋯, the list row's menu and the palette, and F2 on a chat's row in the list opens it.
- The same failure of purlis's own is one report draft, whichever chats meet it.
- "See exactly what's sent" shows what purlis's own report builders make of a sample crash and a sample draft, byte for byte, instead of a description of them (#5688).
- A report draft's Always send is off where your project turns sending without asking off, and says so, instead of offering a press the project refuses (#5696).
- Words an agent adds after Always send sent its draft are sent after it, linked to it, and listed in Settings › You › Reports, instead of being lost (#5695, #5697).
- A report waiting offline counts against the day's five unread drafts on the day it leaves (#5693).
- A Crash report, and a failure of purlis's own that it sent to the Report intake, carry their fingerprint as a tag too.
- Resolve settles a save's conflict in a folder beside the project.
- A setting both machines changed is chosen in the window.
- Saving after the remote moved replays this machine's commits without a rebase.
- Resting on a tab while you drag brings it forward after a moment, and Esc takes you back.
- Placing a pane with the keys shows the same preview as dragging.
- An update of one skill that fails part way now puts the skill's files back as they were, instead of leaving what it wrote.
- A run's notice offers Allow for an ssh jump to a scoped link-local host written as ssh reads it in a jump, `-J u@[fe80::1%%en0]`, as the last hop.
- A chat on a pool's runner is held to where it runs.
- A runner's own policy decides whether a chat it runs may ask about a host.
- A Docker pool's runner is checked against every container setting that could give it more.
- A cluster pool's credential plugins are kept by their full path.
- The cluster operator holds more of what it makes.
- Saving's "Sign commits" switch is now "Sign saves with your own git setup", so it reads apart from Signed commits, which signs the commits the app makes for chats (#5626).
- Browsers: a chat can't close a browser while the person has it taken over.
- Settings' groups and its Edit as TOML files are one page navigation, the one the Skill studio uses: the group shown is marked by the accent and a bar at its start, and the files at its foot are one Tab stop, walked with the arrow keys.
- Seven more built-in skills say when they are for, and which skill to use instead.
- A task you asked for notifies you when it fails, or when it finishes after a long while with you away.
- The palette's row for a chat's skills says what it does: Show skills of … (it said *Skills of …*), and the pane foot's skills warning says the same (#5543).
- Signed commits says what removing another device's key does, and asks first.
- A chat's unsigned commit gets one quiet line, "Not signed: signing isn't set up on this Mac", pointing at the Signed commits card, in place of a warning (#5630).
- Change a chat's skill budget where you see it, for each program.
- The skill editor asks lint extensions as an edit opens, not as you type.
- The built-in skills and a chat's briefing say each rule one way.
- A chat whose profile's key is missing from its vault asks for it in the Start dialog.
- The status bar is the chrome's height, and its project copies its directory.
- A finished chat reads Idle only while its folder holds uncommitted work; with nothing left behind it reads Done (#6014).
- A chat the Skill studio started stays on its skill's Runs once it closes, with when it finished.
- The app's program is now `purlis-app`, not `charter-app`.
- The app runs git only where the repository's settings, hooks and index have one name.
- A push the app makes for a chat is never signed, whatever `push.gpgSign` says: a chat's save of the project and its `purlis worktree push`.
- The ✋ counts what waits on you in every open project, not only the one in front, and says how many of them are elsewhere ("3 things wait on you, +1 elsewhere").
- ⌘T opens an empty tab and ⌘N starts a new chat (Ctrl+T and Ctrl+N off a Mac, where a chat's terminal keeps them for its shell). ⌘⇧T is still New shell.
- An empty pane offers New shell beside New chat, and says you can drag a chat in from the sidebar.
- The project index builds on about half the CPU.
- A chat leaves the project root's git to the app.
- The app runs git only in a git folder whose files are its own.
- A chat with no workspace yet is told only the ten most recently used.
- Settings › Theme's gallery checks only the themes that changed when it is read again.
- Importing a theme takes a fraction of the time it did, a light one most of all.
- Updates is two lists: Mine and Project.
- A chat can ask for a key and its program's Allow in one notice.
- A chat's skill run and its browsers are on its pane head, not its tab.
- A head with no room for its state's word shows its tasks' button as a list, not a second copy of the state's glyph beside the chat's mark (#5524).
- A split tab's picture draws a tile for each pane, so three panes no longer look like four, and past four it is a 3 × 3 grid (#5577).
- A view beside a chat is named by its own title, such as its file's name, in its tab's hover, and the tab takes that title when the view becomes its first pane (#5577).
- Where a chat runs is never guessed.

### Deprecated

- **The charter names stop working at 1.0.** Until then purlis reads its old names and writes
  only the new ones. From 1.0 it stops reading them: the `charter` command (run `purlis`), the
  `CHARTER_*` environment variables (set `PURLIS_*`), a project's committed files under their
  charter names (`purlis doctor --fix rename-plane` moves them), this machine's local state under
  its charter names, such as `.charter/`, `charter.local.toml` and the config and data homes
  (`purlis migrate` moves them), and keychain items under `charter/` (the same migration copies
  them to `purlis/`). The removal waits past 1.0 while `purlis doctor` still sees an old name in
  a project you opened, so run it before you update: it names every old name it finds. A
  migration waits until nothing named charter or purlis is running, and going back to a build
  from before the rename on the same machine needs `purlis migrate --undo` first. A project
  `rename-plane` moved is no project at all to a teammate on a build from before the
  compatibility check, so update every teammate before you move a shared project. Every
  version's notes say this until 1.0 (ADR 0091).

### Fixed

- A row in a pane's band keeps its room inside its edge.
- A sandbox block's Notice shows at once.
- A text box, a select and a combobox have a visible edge in Purlis, so an empty box can be found on the window and in a panel or dialog.
- A browser login notice goes as soon as the chat's call is cancelled, instead of staying for 15 minutes and blocking the chat's next try.
- A secret fill's notice goes as soon as the chat's call is cancelled, as a login's does, instead of staying until you answer it or 15 minutes pass.
- A secret fill waiting for you to add its key stops waiting as soon as the chat's call is cancelled, and its Secret ask goes from the window unless another chat still needs that key.
- A chat's card goes when the keyboard leaves its row.
- A chat shown only in a tab window says its notes once.
- A chat's pane no longer freezes half-drawn after the waiting strip comes and goes.
- A background shell or monitor a Claude Code chat started now ends with the chat's harness when the harness exits on its own or is ended from outside purlis, not only when purlis ends it.
- A chat's browser asks you on the chat's own pane again.
- A chat with no row yet is called its own name, never its tab's.
- What a sandboxed chat makes in its first moments is told.
- From a chat, `purlis skill edit <name> --from <file>` and `purlis skill new` now leave a draft in the Skill studio's Drafts, where you review its diff and accept or decline it.
- A chat that opens while you drag a pane no longer takes the keyboard.
- A secret fill you take the browser over from keeps its mark.
- A browser you watch is the size its view's menu shows from its first frame.
- A shell or a Skill studio chat started in a tab window for another workspace opens in the project window as what it is.
- A manual-only skill that narrows the chat now loads when you name it in a Claude Code chat.
- A link on the launcher stays readable.
- A browser login refused before it ran no longer spends its Allow once.
- A view alone in its tab uses its whole first row.
- A pool runner made again keeps where its skill runs were logged up to.
- A workspace you make is drawn pinned at once.
- Several chats remembering at once are no longer refused as busy for nothing.
- A merge of a merge pushes without being taken for an allowlist change.
- Moving a chat to a pool with git older than 2.40 says which git and what to do.
- A persona's `workspaces/<ws>:<name>` skill that is not there is said on the chat's tab.
- A skill the router hid now loads on Claude Code when it narrows the chat.
- A pane's band goes in one step when its last warning does.
- A chat's pane that froze while you typed comes back on its own (#5478).
- A pane's trouble notice draws as trouble.
- A save request that conflicts with its target now recovers by itself (#5338).
- A save in `push` mode no longer stops on a memory index both machines added to.
- A run on a vault its persona may not use no longer offers that vault's own entries, unless its notice offers the vault's own Allow.
- A browser site you remove is asked about again.
- A browser reopened from kept logins reaches its sign-in host from the start.
- A bug report masks the name of a clone in a workspace's `repos/` folder, as it already did for a clone at the workspace's root.
- A move that does not finish no longer hides a change.
- A menu row and a list's option are named by their words alone, and say the rest once.
- A run of another workspace's skill a persona names is that workspace's copy's.
- A runner chat's skill runs reach the Skill studio after the window was closed.
- A runner goes only with its work fetched.
- A fresh runner's check no longer holds up other chats' branches coming back.
- On a pool's runner, a persona's tool nobody allowed says the Allow is wanted too.
- The Activity tab decides which reports are purlis's from the task's record, never from its words.
- A task renamed with a backtick or one of purlis's marks keeps its report's heading whole.
- A tab window keeps its strip when you press a chat from another workspace.
- A tab window keeps its strip when a chat from another workspace starts or comes back there.
- A theme's colours for a tab's × no longer cover its look under the pointer.
- A task's report reaches a busy app.
- A tab's tasks menu no longer opens under a pointer that has not moved.
- A vault grant on this machine is no longer dropped by an unrelated write.
- Allow stays told apart from the suggested answer whatever state either is in.
- The Allowed list drops an Allow once as soon as it goes.
- An agent identity's vault stays registered until the identity is removed.
- A browser's start no longer fails on one bad answer from a server.
- An older runner that was stopped, or whose container was removed outside purlis, is no longer removed with its home unchecked. purlis can't ask a stopped runner what it holds without starting it, so the runner stays.
- A screen reader hears every line a list's option draws about it.
- ↑ and ↓ at the edge of a wrapped line stay in the box (#6045).
- The window no longer waits while purlis reads what is asked of you.
- A pane's note that is shown nowhere else can no longer be closed by mistake.
- A skill brought back under a new name leaves Deleted.
- The keyboard reaches a taken name's choice.
- Your clicks in a browser's view land where you point.
- A card row's ⋯ opens its menu as a menu button does.
- A chat starts faster in a project with many personas.
- A chat starts faster in a project with many workspaces.
- A chat that gets none of the plane's skills keeps your own skills' descriptions too.
- A Codex chat is told its whole start briefing.
- A Codex plugin its `config.toml` turns off is no longer listed as in the chat.
- A Codex chat is told its skills whole.
- A control's tooltip keeps its width at the window's edge.
- The Curate menus no longer take seconds on a big project.
- A deleted skill no longer counts as installed: its source's page offers it again, and it stops being offered updates.
- Dispatch commands no longer say "the purlis app did not answer" while the app is only busy.
- Each sandboxed Claude Code chat has a temporary folder of its own.
- Code in a diff reads at 4.5:1 in Purlis and Purlis Classic, on a changed line and on a changed word, in every appearance.
- Ending a chat ends the background shells it started.
- A login follows the same value rules as a fill (#5272).
- The save status in the title bar reads only what changed, on a big project too. git's untracked cache was never used for it, and a project purlis had not saved yet had no cache and no file monitor token, so each look read the whole tree.
- The Personas view's In this workspace keeps who worked there after a relaunch.
- The Inbox and the Notices update their ages with the rest of the window.
- A chat starts faster wherever the project index is used.
- Keep blocked holds at once.
- Keep blocked on a vault the persona can already use says so.
- Keep blocked on a run's notice says when it can't stop the chat.
- A dispatched task held on an approval prompt says so.
- A sandboxed chat's `purlis worktree add` no longer tells it to enter the new folder.
- A LIVE workspace's files say what git will actually commit. *This machine only* and *Saved with the project* are now git's own answer, so a file git still tracks is marked saved, and a nested `.gitignore` or `info/exclude` is honoured.
- On macOS, a chat's notification now changes in place.
- Listing a memory store looks at each file once.
- A branch folder whose commits merged in a pull request now counts as finished.
- A branch whose folder is already gone is no longer left behind.
- Move into repos/ survives a crash partway.
- A note you put away in a tab window stays away in the others.
- A sandboxed opencode chat on a profile purlis made keeps its data in that profile.
- An ended task's `↳ from` goes to the chat that asked it.
- A theme that fills a tree or a table is held to what stands in its rows.
- A plane's root is never left part-way by a save.
- Two machines that each remembered something or allowed a host no longer stop the plane's save.
- A chat's pool and secret asks wait for you on the chat's own pane too.
- `purlis worktree prune` from a chat now answers on a large workspace.
- The push scan no longer refuses a save over numbers purlis writes itself.
- A merge of a branch from before the scan's allowlist existed is no longer refused as an allowlist change.
- A push from a bare repository to the remote's address leaves out what the remote already has, as a push from a clone does, instead of scanning the whole history again (#5822).
- A chat that names its conversation as it starts can always be restarted.
- Restart now on a chat waiting on its own background work goes ahead and tells the chat.
- A chat that dispatched tasks is told about them after a restart.
- `purlis dispatch wait` returns a report the app no longer holds in memory.
- After a quit, a chat is told what its background shells last printed.
- Every reopen tells a chat what the restart ended, not only a quit.
- A chat that started background work in a turn that purlis's quit or update cut off is now told about that work when it is reopened.
- Quitting or restarting to update warns about a chat in a background turn.
- Start fresh warns about a chat waiting on its own work.
- A chat waiting on its own background shell is told when a restart ended it.
- A row's own controls keep their keys in the Inbox and every list of rows.
- A row's tooltip can be hovered, and comes back after Esc.
- Keep blocked on a refused vault holds against a run already on its way.
- A clone a runner's chat made comes back without a second copy or a stuck move.
- A clone a runner's chat made comes back into `repos/`.
- A second session host no longer starts beside one whose lock file the system cleaned away.
- A GitHub App token is never kept for a chat that ended while its run was being served on this machine.
- Claude Code chats are held from writing a program held by its own name, as wrapped chats already were.
- The save status is right on a very big project on a slow disk.
- A Secret ask's record keeps its order.
- A session host that stops answering no longer piles up secret asks' steps in memory.
- Secret fill: what the person typed while driving a browser no longer stops a later fill in it.
- Secret fill: a site whose name holds a login's user name stays visible to asks, and can be filled, once that value has been filled on that site.
- A page on the site a browser signs in on is told as one even where its address holds a value kept out.
- More spellings of a kept value are taken out of what a chat reads: letters whose capitals are spelled differently, a Latin-1 address, and a `+` read as a space.
- Secret fill: no fill goes to a browser run by an older purlis on a runner or host.
- A custom theme's switcher rows and plain count are read as they are drawn.
- Shift+Tab back into a list lands on the row you left, not on a button or box in it (#6033).
- An ended task in a tab behind the one in front is offered Go to … in the palette, while the chat that asked it is open, as one in the tab in front already was (#5543).
- Set up signing's Check no longer tells you to add a key that's already on your account.
- The Skill studio no longer says a skill's script calls a program because the word appears inside a quoted string or a here-document, such as `sort_by` in a jq program or `body{font:…` in CSS.
- A skill run's chip on a pane head is named by the words it shows, so a screen reader and voice control both find it by what's on screen (#5614).
- Smart close and the memory skill work in a sandboxed chat.
- A chat's ssh connects on Fedora and RHEL.
- The Start dialog's Secret ask holds every start while you answer it (#5670).
- The Start dialog says to type a refused value again.
- The status line answers faster on a big project.
- One Tab leaves a list from its row, and Tab walks a row's buttons both ways (#6071).
- Faint text in the terminal is readable in every appearance.
- The ✋ no longer counts a chat that isn't waiting on you.
- The Skill studio shows one copy of a skill you have two of, as your chats do.
- `purlis theme check` says when a theme's side-view cards blend into their panel or a text box loses its edge, even when the theme sets no component parts.
- Text boxes have a visible edge in Purlis Classic, at 3:1 or more on every ground they stand on.
- `purlis theme check` says when a theme's colours lose a meter's bar on its track, even when the theme sets none of the meter's parts: the score, the baseline and the previous version, each at 3:1 on the track.
- Tab leaves the title bar's pinned workspaces from the chip you arrowed to.
- Undo a turn and Discard keep what the file was.
- The save status in the title bar no longer reads every file twice on a big project.
- Closing a tab behind the one in front leaves the keyboard where it was.
- A narrow pane's head draws a chat's state once.
- A folded pane that needs you shows one ✋.
- In Classic, the × of a tab under the pointer stands on the tab's own hover colour, not on a lighter box (#5524).
- A screen reader finds its way round the window by its landmarks.
- A write to your workspace files lands in the folder purlis checked.
- A branch folder whose pull request closed with its head on origin is removed without `--force`.

### Security

- **A blank window opened by a page of the site a login signs in on is kept off with it.** Until
  the new window's own page says where it is from, the chat reads and acts on it only as it may
  on the page that opened it.

- **A download holding what you typed in the browser stays out of the chat's folder.** Saving a
  download into a chat's folder is refused where the file holds a text of 8 or more characters
  you typed into that browser, in any case or in the forms a chat's reads already take it out
  in, as it is for a secret purlis filled. You can still save the file yourself.

- **A fill's or a login's notice is no longer raised for a key that is a link into a vault its
  chat may not use.** The chat is told at once which key. A link made while the notice waits is
  caught when you press: nothing is allowed, no line is written, no vault is given to the
  persona, and the notice says why (#5831).

- **A page holding a filled secret asks before its server's redirect takes it to a site you
  allowed for something else.** Only your answer to the ask about that page leaving counts: an
  earlier Allow of the same site, when the browser asked to go there for another reason, no
  longer lets the redirect go by itself. Each browser asks at least once.

- **A download that holds a secret purlis filled is not brought into a chat's folder.**
  `browser_save_download` refuses a file that holds a value purlis filled into that browser, or
  signed in with, in any form a chat's reads already have it taken out in. A download whose name
  Chrome may have shortened by its length in bytes is named `download`, like a name purlis
  shortens itself.

- **Allow for this chat and Always allow no longer fill a field whose form sends it in the
  page's address.** Where the form a field is in submits by GET, by its own method or a submit
  button's, a fill under either grant is refused and the chat is told why: the address would
  carry the secret into the site's logs and the browser's history. Asked about, Allow once
  still fills it. purlis checks this again as it writes the value, so a page that changes its
  form in between is caught.

- A run you allowed under a Run grant no longer takes the chat's database and cache hosts and ports, its cloud region or its certificate authorities from the environment (`PGHOST`, `MYSQL_HOST`, `REDIS_HOST`, `AWS_REGION`, `SSL_CERT_FILE`, `CURL_CA_BUNDLE` and the like). Where it connects is what the command you read says. A certificate authority set only in the chat's environment doesn't reach that run, so name it in the command. A key that any persona's browser login or secret-fill line holds is never offered to a program, whichever persona's vault it's in.

- **A picked entry is linked only where every persona of the vault may use it, as it is
  picked.** A Secret ask's or a run's picker no longer offers, and its press refuses, an entry of
  a vault that another persona of the asked vault may not use, and the refusal names that
  persona, since its tools would read the entry through the link (#5745).

- **A link is judged each time it is read, not only when it is made.** A declared tool, a run the
  person allowed, a runner chat's tool, a browser fill, a login (declared or granted) or a chat
  profile's sign-in that would read through a link into a vault its persona may not use is
  refused, and the refusal names the key. It covers links made earlier and personas allowed a
  vault after its link was made. If purlis can't read this machine's vault grants, it reads
  nothing through a link into another vault (#5795).

- **A browser login or secret fill keeps more out.** A fill of a value, or a login with a password,
  shorter than three characters is refused before anything is typed, saying why, since a value
  that short cannot be kept out of what a chat reads. A login checks the field's own document is
  on the declared origin before and after it types. On Linux, what the person pastes with a
  middle click while they drive is kept out as a paste is. A native drop is judged by the frames
  the page shows as it drops.

- **A browser login holds its page's related windows as a secret fill does.** A login fills
  nothing while a window on its site, or on none yet, is related to its page, and a related window
  on another site closes once the page loads another document or closes; the chat is told it did
  (the person's ruling of 2026-11-05).

- **A chat narrowed by a handoff or a dispatch fills and logs in with only the grants it was
  narrowed to.** Its secret fills and browser logins read the Always allows and declared logins
  of the persona it was narrowed to, not those of the persona it was started as, and its
  notice's Always allow writes in that persona's tools. The vaults it reads are still its own
  persona's.

- **A browser's page whose address holds a secret purlis filled is named by no site.** Its view
  shows no site above it, and nothing purlis records of what you do in it names one. A click on
  a page scrolled down is judged by what is drawn where it lands, so one that would land on a
  frame of the site a browser signs in on is refused there too.

- **A field the page marks for a card or payment is asked about each time.** Where a page marks
  a field `autocomplete="cc-…"` (a card's number, its expiry, its code), filling a secret into
  it raises the notice with Allow once and Keep blocked alone, under Allow for this chat or
  Always allow too, whether or not its key is marked `card = true`.

- **Revoking `use` on a GitHub App key now stops minting for the Pools its holder wrote.** In the
  cluster operator, a Pool that names a key carries `runners.purlis.dev/forge-writer`, set by
  whoever writes its forge to their own name in that write, and the operator asks before each
  mint whether that person may still `use` the key, by name: a right held only through a group is
  refused. **Upgrading: a Pool written before this version mints nothing until someone who may use
  its key names themselves on it** (`kubectl annotate pool <name>
  runners.purlis.dev/forge-writer=<their name>`) (`docs/cluster-operator.md`).

- **No writer adds to a purlis pool's runner pod through its upstream claim.** The cluster
  operator's `purlis-claims-use-their-pool` policy now refuses `env`, `lifecycle` and pod labels or
  annotations on a SandboxClaim of a purlis pool's warm pool, from any writer, not only from the
  operator. A claim already holding one is refused its next write; `docs/cluster-operator.md` has
  a query that lists them before you upgrade.

- **A refused Docker engine is named by its scheme alone.** When a pool's `DOCKER_HOST` points
  at an engine purlis cannot tell is on this machine, the refusal says "a tcp:// address", never
  the address itself, which may hold a user and a password.

- **A runner whose home is older than its Pool's GitHub App key gets no token.** The cluster
  operator marks a runner's upstream claim, as it makes it, when its Pool names a key, nobody can
  change that mark after, and the operator mints only for a runner whose claim carries it, so a
  kept volume from before the key is never a minting runner's home. **Upgrading: a kept runner
  made before this version is refused tokens until its Claim is made again**, and so is a fresh
  one still running from before, until its Claim is made again or its runner goes idle and runs
  again (`docs/cluster-operator.md`).

- **A frame sandboxed without its own origin is judged as having none, even at a web address.**
  The chat's browser types nothing into it, and purlis fills no secret into it, whatever site
  its address names: the sandbox gives the document no origin, so purlis can't say where what
  goes into it ends up.

- **A screenshot covers more of what a browser kept out.** A code typed across boxes is found
  with the same dot, slash or bullet between each of its characters, and prices, dates and
  versions are not taken for one. Two kept texts side by side across an element's edge are each
  painted over, and so is an SVG `use` that may draw a kept text. A text shadow or drop shadow
  on what is painted over widens the paint by as far as the shadow reaches. A screenshot is
  withheld when the page draws a backdrop through a filter of its own, or draws a field or text
  it keeps out through a filter of its own or a shadow whose reach cannot be read.

- **A secret is filled only with an entry its persona may use.** "Use an entry I have" on a Secret
  ask now lists only vaults the asking persona may use, and leaves out entries a browser login
  fills or a `mint` line marks, as a run's notice already did. A pick is checked again when you
  press, and a link is never made into a vault the persona may not use. A program you allow on a
  run's notice is never handed a key that links into a vault the persona may not use, whenever
  that link was made, so the notice can't hide where the value comes from (#5246).

- **A 2FA seed in any mix of upper and lower case is not filled into a page.** A value that
  looks like a seed counts as one whatever its case, so a seed with its first letter
  capitalised, or in random case, needs the no-seed mark, as one in a single case already did.
  A seed joined to a name before it by `_` or `-` (`secret_GEZD…`) counts too. A password of 16
  or more characters drawn only from base32's alphabet (the letters, and the digits 2 to 7) now
  needs that mark as well: one with a 0, 1, 8 or 9, or any other mark, still fills.

- **A sign-in's download is its own page's.** While `browser_login` signs in, a download from the
  site it signs in on, or from no site purlis can tell, is brought into the chat's folder only
  where the page it signs in on, or a window related to it, began it; one begun on another tab is
  kept out as that tab is kept off the site.

- **A frame of the site a login signs in on takes no input inside another site's frame.** A click
  aimed at a frame of that site, drawn inside a frame of another site that runs in a process of
  its own, is refused as one on the frame itself is.

- **A site whose name holds what you typed in the browser is not named to the chat.** Where
  the browser held a request back to ask you about a site, and that site's name holds a text of
  8 or more characters you typed into any of its pages, the chat is told only that a request
  went nowhere, as it already was for what you typed while driving the browser.

- **A clone whose `.git` is a symlink is no longer sent to a runner or wired as a guest tree**,
  in a workspace's `repos/` folder or at its root. purlis wrote its exclude block through such a
  link, which could name a repository outside the plane. The repo list already refused one.

- **A runner's keep takes out the MCP servers a chat added to a project** in Claude Code's state
  file, and a runner that cannot read who listens for a sign-in no longer passes the browser's
  answer on: the sign-in says to sign in again.

- **A keep refused for a linked settings folder names the folder** and says to make it a plain
  folder on the runner, then keep the profile again.

- A sandboxed chat no longer writes gh's or glab's settings and extensions where your environment
  moves them (`GH_CONFIG_DIR`, `GLAB_CONFIG_DIR`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`), as it
  already could not at `~/.config/gh`.

- A runner clears opencode's `package.json` and `node_modules` in a kept profile's settings
  folder each time it keeps it, so a chat there leaves nothing for the next one's plugins to
  load. opencode makes them again as it starts.

- **More of what runs later is kept from a chat.** No chat writes a git folder's glab settings
  (`.git/glab-cli`) or git's older remote definitions, nor the project's `inventory/`, whose URLs
  the window's Clone by name uses. purlis's own git turns off a config-named hook from your global
  or system config too. A declared tool runs with a vault's value only where each program the
  repository's git config names by a path is out of a chat's reach, and a declared git never
  carries on a rebase, cherry-pick or revert.

- **A Claude Code chat starts only where its sandbox can hold every repository.** It is not
  started where a repository's path has `*`, `?`, `[` or `]`, or where a folder outside the
  repositories holds more than purlis looks through; the refusal names the folder.

- **No grant reaches the system's name lookup** by Linux's system message bus,
  `com.apple.mDNSResponder`, or port 53 or 853.

- One run on a vault its persona isn't tagged for raises one ask: a run you're asked to allow no longer also offers to let the whole persona use the vault. Keep blocked on a refused vault now lasts for the rest of that chat, so the chat can't bring the question back by trying again.

- **The cluster operator's mint port answers a runner only from its own pod, and paces it.** A
  token is minted only for a request coming from one of the runner pod's own addresses, at most 5
  times a minute for a Claim, and one address may hold at most 4 of the port's connections at
  once. A token whose answer could not be written to the runner is revoked. A network that
  rewrites pod source addresses between pods cannot use the mint port (`docs/cluster-operator.md`).

- **A cluster runner's sign-in goes to the runner purlis paired with.** The window now signs a
  profile in on a cluster runner over its own link to it, which proves the runner by its key. A
  runner on an older purlis, and a sign-in from the command line, still go through `kubectl
  exec`. Those now read the pod's uid right before the sign-in starts and again once the sign-in
  has reached it, before it shows you anything or passes anything on. If a new pod took the
  runner's name in between, the sign-in stops and none of it counts.

- **A stopped Claim or minting Pool stays stopped until someone entitled runs it again.** In the
  cluster operator, only a Claim's requester can set its `stopped` back to `false`, and only
  someone with `use` on a Pool's GitHub App key can do so for a Pool that mints, so revoking
  `use` on the key takes it away from the Pool's other writers. Turning a Pool's minting on now
  takes `use` on its key too, since it adopts the image the Pool already names. A namespace that
  was given the operator's label by hand before its policy was installed can now finish
  deleting, and `purlis-cluster-operator doctor` names any namespace outside
  `purlis-runners-*` that carries that label, with the command that removes it.

- **The cluster holds the operator's templates and kept volumes to what their Pool says.** A new
  admission policy, `purlis-operator-writes-what-its-pool-says`, refuses the cluster operator's
  service account a pool's template whose containers run another image than the Pool's, and a
  kept claim's volume in another storage class than the Pool names, or than the one it was made
  in. It is kept when the chart is uninstalled, as the other policies are.

- **A run's notice offers no Allow for an ssh jump whose brackets hold anything but an IPv6
  address** (`-J a,[.-_]`), or whose user holds a bracket (`-J [x]@host`), since ssh pastes a
  jump list unquoted into the command its shell runs (#5892).

- **A run's notice offers no Allow for a command whose flags name a command.** `ssh`, `scp`,
  `sftp`, `rsync`, `kubectl`, `psql`, `mysql`, `mariadb`, `zip` and `wget` are still offered
  Allow once on the exact command, by any name their builds go by (`kubectl.docker`, `psql-16`,
  `wget2`), but not with a flag that names a command or a file that can: among them
  `ssh -o ProxyCommand`, `-o XAuthLocation`, `-F` and `-I`; `scp -S`; `sftp -b`; `rsync -e`,
  `--rsh` and `--rsync-path`; `kubectl --kubeconfig` and kustomize's plugin flags; `psql -f`, `-o`
  and a backslash command in `-c`; `mysql --pager`, `--defaults-file` and a shell command in `-e`;
  `zip -TT`; and `wget --use-askpass`, `-e` and `--config`. The chat is told to leave the flag
  out. `sed`, `aria2c`, `kustomize`, `sshpass`, `autossh`, `mosh`, `lftp` and mysql's
  server-side and shell-like tools get no Allow at all. A persona can still declare any of them
  as a tool (#5799).

- **A Run grant is offered for fewer programs that run others.** GNU builds by their `g` or
  `gnu` name (`gm4`, `genv`, `gnumake`), git's own programs (`git-shell`), versioned and
  cross compilers (`gcc-13`, `clang-17`, `x86_64-linux-gnu-gcc`) are refused like the programs
  they are, with no notice raised. `curl` is still offered (#5424).

- **More programs that run others get no Allow from a run's notice.** These each run a command
  that a flag, an address, a template or their own words name, or what a file a chat wrote says:
  `xcodebuild`, `msgexec`, `leaks`, `pyenv`, `erb`, `helm`, `socat`, `ncat`, `nc`, `netcat`,
  `tar`, `bsdtar` and `msgfilter`; the secret injectors `op`, `aws-vault`, `doppler`,
  `infisical`, `sops`, `chamber`, `envconsul`, `summon`, `envchain`, `dotenvx`, `dotenv`,
  `teller`, `berglas`, `aws-sso`, `aws-okta`, `saml2aws`, `awsume`, `bws`, `phase`, `granted` and
  `assume`; the environment managers and project runners `rbenv`, `asdf`, `mise`, `rtx`,
  `direnv`, `pipenv`, `conda`, `mamba`, `micromamba`, `volta`, `goenv`, `jenv`, `fnm`, `nix`,
  `nix-shell`, `devbox`, `pixi`, `hatch`, `pdm`, `rye`, `tox`, `nox`, `helmfile`, `bunx`, `tsx`,
  `pytest`, `pre-commit`, `pulumi` and `cdk`; the programs that run what a file a chat wrote says
  `terraform`, `tofu`, `terragrunt`, `ansible`, `ansible-playbook`, `vagrant`, `packer`, `cdktf`,
  `sst`, `serverless`, `jest`, `vitest`, `mocha` and `rspec`; and `xctrace`, `valgrind`,
  `hyperfine`, `watchexec` and `entr`. A chat's run of one with a vault's key is refused at once,
  with no notice. A persona can still declare one as a tool (#5619).

- **A run's notice offers no Allow for MySQL's and MariaDB's hotcopy, secure installation and
  backup tools** (`mysqlhotcopy`, `mysql_secure_installation`, `mariabackup` and their `mariadb-`
  names): each runs a program, a script or a plugin its flags name. A persona can still declare
  one as a tool (#5891).

- **A run's notice offers no Allow for MySQL's and MariaDB's server, router, `mysql_plugin` and
  `mysql_upgrade`, Percona's backup tools or the server's state transfer scripts** (`mysqld`,
  `mariadbd`, `mysqld_safe_helper`, `mariadbd-safe-helper`, `mysqlrouter`,
  `mysqlrouter_plugin_info`, `mysqlrouter_passwd`, `mysqlrouter_keyring`, `mysql_migrate_keyring`,
  `mariadb-plugin`, `mariadb-upgrade`, `xtrabackup`, `innobackupex`, `xbstream`, `xbcloud`,
  `xbcloud_osenv`, `xbcrypt`, MariaDB's `mbstream` and every `wsrep_sst_` script): each runs a
  server, a client, a plugin or a program its flags name. A persona can still declare one as a
  tool (#5931).

- **Still more programs that run others get no Allow from a run's notice.** Programs that run
  the command after them or one they name: `consul-template`, `railway`, `aws-runas`, `vals`,
  `gomplate`, `envdir`, `chpst`, `systemd-run`, `run0`, `telepresence`, `firejail`, `flatpak`,
  `flatpak-spawn`, `rr`, `pkgx` and `flox`. Programs that run what a file a chat wrote says:
  `task`, `skaffold`, `tilt`, `dagger`, `earthly`, `act`, `firebase`, `netlify`, `vercel`,
  `wrangler`, `chezmoi`, `mage`, `invoke`, `fab` and `cdk8s`. Build tools and test runners: `sbt`,
  `mix`, `playwright`, `cypress`, `gitlab-runner`, `circleci`, `stack`, `cabal`, `dune`, `zig`
  and `lein`. A chat's run of one with a vault's key is refused at once, with no notice. A
  persona can still declare one as a tool (#5833).

- **A sandboxed chat keeps the folders above what it may not write in place.** A folder granted
  to a chat, and opencode's data folder, can no longer be moved aside to write under another name
  what the sandbox denies. Claude Code's sign-in on a runner opens only a page asking for Claude
  Code's own client and code page. A runner clears Claude Code's `hooks` folder from a kept
  profile, never clears a settings folder reached through a link, and clears what an earlier
  version left behind without holding up other profiles.

- A sandboxed chat can no longer write gh's or glab's folders named by a relative `GH_CONFIG_DIR` or `GLAB_CONFIG_DIR`, nor the later-code names under a moved `XDG_CONFIG_HOME` or `XDG_DATA_HOME`. At the floor, the folders above each held name in your home stay pinned, also while a name is allowed for one chat.

- On a runner, a wrapped Claude Code's plugin data folder no longer lets a chat write the names the floor holds, and a start refused late leaves its config folder as it was. purlis's own tidying of a profile's folders never follows a folder swapped under it.

- **What a profile sends to a runner is read in more of the ways a credential is written.** An
  address's signing query value (`?api_key=…`) is emptied wherever it is, an address written with
  JSON's escaped slashes loses its user information, a Codex server whose address is signed goes
  whole, and so do permission rules, `--key`, `-p` glued to a MySQL password, and settings named
  for a credential (`apiToken`, `GH_PAT`). Codex's provider query parameters and a profile's own
  shell policy variables are removed, and an `.mdx` file is filtered as Markdown. A script or
  other file sent as written that holds a value shaped like a token is refused in words, naming
  the file, so nothing is sent until the key is moved to a vault.

## [0.5.0] - 2026-11-05

0.5.0 is the first release as purlis, and the first under the Apache License 2.0. The app,
its command, its environment variables and its keyring items now carry the purlis name, and
the charter names keep working until 1.0. Every chat runs in a sandbox, on Claude Code, Codex
and opencode alike, at least at the floor level. Chats can now run on a Docker pool or in your
team's cluster, drive a real browser, and dispatch tasks that work on their own branches and
report back. The Inbox gathers everything that waits on you, and the Skill studio writes,
tests and grades skills. It also carries 0.4.2's hardening, which was never published on its
own.

**After updating:** if your personas had sub-agents, run `purlis doctor --fix persona-agents`
in each project (see *Removed*).

These are the highlights. All 2,441 changes are in
[Every change in 0.5.0](https://github.com/purlis/purlis/blob/v0.5.0/docs/changelog-0.5.0.md).

### Added

- **Install from a terminal.** `curl -fsSL https://raw.githubusercontent.com/purlis/purlis/main/install.sh | sh`
  installs the latest stable release on macOS (Apple silicon) or Linux (x86_64), and
  `PURLIS_CHANNEL=dev` installs the dev build. On macOS it avoids the first-launch refusal an
  ad-hoc signed app downloaded in a browser meets. When `gh` is signed in, it checks each
  download's build provenance first and installs nothing that fails.
- **A chat can put a vault secret into a field of its browser, with your yes.**
  `browser_fill_secret` names a field from the page, a vault and a key; you see the key, its
  vault, the exact site, the page and the field's label on one notice, and choose Allow once or
  Keep blocked. purlis fills it only into a one-line field whose own frame is on that site at the
  moment of filling, and the value never reaches the chat (#5115, spec #5100).
- **Run a workspace's chats in your team's cluster.** The add pool dialog has a Kind: Cluster
  takes your kubeconfig's context and the name of a Pool of purlis's cluster operator, shows the
  cluster operator's key for you to check against the one your cluster's admin gives you, then
  claims a runner there as you, pairs it and keeps it. A workspace set to run on that pool runs
  its chats on that runner. Before every link purlis checks that the Pool still names the key you
  checked and that the operator still vouches for the runner it paired with; either changed is
  refused and asked about, as a Docker runner's changed key is. purlis runs your own `kubectl`,
  found where its installers put it. The pool keeps its own copy of the context, taken when it
  was added; to pick up a changed kubeconfig, add the pool again (#3292).
- **A workspace's chats run on a Docker pool** (#2046). Once this machine keeps a pool, a
  workspace's settings offer **Runs on**: this machine, or a pool. A chat started in a workspace
  set to a pool runs on the pool's runner and behaves as a local one: its tab, input, asks and
  stop work the same. The workspace's code goes to the runner as the chat starts, with your
  uncommitted work. The runner's branches come back when a chat there ends and when you return
  to the window. Changes lists them under each repo, with the commits not on this machine. Chat numbers
  never repeat between this machine and its pools. Shells still run on this machine.
- Agent identity, from the palette: give chats their own GitHub identity with GitHub's own page, with no file edited and no terminal. Its row says what it acts as and on how many repos, and removing it takes the tool lines, the key and live tokens at once.
- **Chats drive a real browser.** Every harness's chat gets purlis's `browser_` tools: a
  snapshot of a page with a reference for each element, clicks, typing, drags, waits and a
  screenshot the model sees as an image, on a browser purlis runs and no script the model
  writes. A chat's first browser tool asks you once what its browser may reach: this project's
  sites and its dev servers, also the open web, or Keep off. A project's chats hold at most
  three browsers together, and a browser's reach follows its chat's reach, including what you
  withdraw (#2013).
- **Chats survive a crash of the app.** With chats running in the session host (the default,
  #2004), a crash or a kill of the app no longer ends them: the session host keeps every chat,
  its network and its project's saving for ten minutes. Start purlis again within them and every
  tab comes back on the same chat, with what it wrote meanwhile on screen and each chat's state
  and Needs you as the host kept them. After ten minutes the host ends them as a quit would, and
  the next start offers to reopen them. Quitting still ends every chat, and so does an update
  (#2006, #646, #2194).
- **Sandboxed Codex and opencode chats on Linux.** A project that runs its chats sandboxed now
  starts Codex and opencode chats on Linux inside purlis's own sandbox, built from the same
  policy as on macOS, where before they were refused. It needs no extra package, privilege or
  namespace, so it works the same on a Linux desktop, in a Docker container and in a Kubernetes
  pod. It needs Linux 6.12 or later (Landlock version 6). In a project with `mode = "on"`, an
  older kernel refuses the chat and says how to go on; elsewhere the chat starts without the
  floor and its tab says why (#1040).
- **Create a project on GitHub or GitLab, in one step.** The welcome's **Create a project** now
  asks where the project lives: **GitHub**, **GitLab** or **Only this Mac**. On a forge, give it
  a name, an owner (you, or one of your organisations or groups), private or public (private
  unless you say otherwise) and a stack template or none. **Create project** makes the repo with
  your purlis sign-in, lays out the project, commits it, pushes the first commit through
  purlis's own credential helper and opens the project, showing each stage as it runs. Without a
  purlis sign-in to that forge, the page asks you to sign in first, and the guided steps add their
  sign-in step. **Cancel**, or a stage that fails, takes back what this creation made: the repo on
  the forge and the folder, and only those (#1819). GitHub deletes a repo only for a token with
  the `delete_repo` scope, which purlis does not ask for; when it cannot delete the repo it made,
  purlis names it and the page where you delete it.
- **A chat can dispatch a task to its own persona, and gets the report on its next turn.**
  `purlis dispatch --name "<task>" <<'BRIEF'`, or purlis's `dispatch` tool, starts a chat of its
  own for the task: as the asking chat's persona, on its harness profile and in its folder, with
  the sandbox the project gives that persona. What you allowed the asking chat alone, and a
  start without the sandbox, are not carried to it. Its first message says which chat asked and
  that the brief is a request from a chat. In the explorer it is listed under the chat that
  asked for it. It finishes with `purlis dispatch report --outcome done|blocked|failed "<text>"`,
  and the chat that asked reads the outcome, the text, what changed and the session record's
  path on its next turn, quoted as data. A dispatch to another persona starts nothing until you
  allow the pair (see "A dispatch to another persona waits for your answer"). Where purlis's hook can tell a sub-agent made the call, it is refused
  and told to return to its chat. One chat waits on at most 6 tasks, a lineage holds 16 chats
  that still owe work, and a chain goes 3 deep (#1436).
- **The doctor fixes what it can, from the terminal and from the window.** A doctor finding that
  purlis can fix carries a fix id, which `purlis doctor --json` prints as `fix` on that row.
  `purlis doctor --fix <id>` applies that one fix. Bare `purlis doctor --fix` applies every fix
  the rows offer, on top of its plugin and ask-rule repairs. Each fix says what it changed or why
  it was refused. The Doctor dialog has a **Fix** button on those rows, which applies the fix and
  checks again. The first fix is `reinit`, offered when the project is missing a baseline folder.
  It adds what is missing and never removes or replaces your content. Removing a git index lock
  is never a fix (FX-1, #1224).
- **Drag panes to split, swap and move them.** Drag a pane by its head, a chat's row in the
  sidebar, or a tab pulled down off the strip. Drop it in a pane's outer quarter, by an edge, to
  split on that side, or in its middle to swap. Drop it on the strip to make a tab. The result is
  drawn before you drop. A place that would leave a pane under about 40 columns by 8 rows is
  refused, with its reason. Holding over a tab brings it forward. Esc cancels the drag. After a
  drop, a split or Tile all, Undo or ⌘Z puts the last layout back. A split's separator now stops
  at the same minimum, and double-click or Enter on it makes both sides even (#4763).
- **Your Claude Code, Codex and opencode folders are now profiles, with nothing to do.** When
  purlis starts after the upgrade, each harness folder this Mac already uses becomes a profile in
  place, named after its harness (*Claude Code (this Mac)*): the same folder, so the same sign-in,
  settings and conversations. Nothing is copied or moved, nothing in the folder is changed, and
  you are not asked to sign in again. Every chat resumes its own conversation, and profiles in a
  project's `charter.local.toml` work as before. `purlis profile list` shows each adopted profile
  with its folder (#2033).
- **A chat can make your theme** (#2333). Ask any chat for a theme, or use **Create a theme** in a
  workspace's or the plane's Curate menu, or **New theme** and **Edit with a chat** in Settings ›
  Theme. purlis's theme skill builds it from a brand colour, an image, a Figma frame, an
  imported theme or another theme. The chat works on a theme draft that the window draws live,
  and checks it until the check is clean, or tells you which warnings are left and why. You keep
  or revert it from the draft's strip. Each of these opens the chat with its prompt typed, ready
  for you to read and send.
- **Move a chat to a pool's runner and back.** A chat's tab offers *Move chat … to pool …* and,
  on a runner, *Move chat … here*. The move is prepared while the chat keeps working and happens
  at its next pause between turns, never mid-turn: its branch, uncommitted work and conversation
  go with it, and its workspace's chats run there from then on. A move that fails leaves the chat
  where it was and says why. Claude Code chats move today (#2048).
- Pool setup: a guided tab, opened from the palette, that finds Docker, recommends it and makes a Docker pool step by step, with no terminal. The other steps show what purlis finds and what to do next for now. The add pool form stays until the whole Docker path is in.
- **Codex chats run sandboxed on macOS.** In a project with `[sandbox]` `mode = "on"`, a Codex
  chat now starts inside the sandbox purlis writes, as an opencode chat does, instead of being
  refused (#1123).
  - **Codex's own sandbox is off inside it.** macOS applies one sandbox to a process, so Codex's
    own cannot run inside purlis's. purlis's holds what Codex's held, and what Codex's could
    not: a folder moved aside, changed and moved back, and a folder swapped for a link.
  - **A Codex home of the project's own.** A sandboxed project's Codex chats keep their Codex
    state in a home of their own, outside the project, seeded with your Codex login. They
    neither read nor write your own Codex home, so nothing they write in Codex's own state
    reaches a conversation you resume outside the sandbox. Files they write in their own
    folder, such as an instruction file, are still read by a later Codex started there.
  - **Writes.** The chat can write its own folder and a temp folder of its own. In the
    project's Codex home, only what a turn writes: its sessions, history, state, log and a
    refreshed login, and the memories, goals and queue Codex's own screen needs. So one
    sandboxed Codex chat can influence what a later sandboxed Codex chat of the same project
    loads, but never your own Codex state. It cannot write Codex's config, skills or
    plugins.
  - **Trust.** purlis marks each chat's folder untrusted for Codex, so Codex loads no
    project-local config, hooks or exec policies there and asks nothing, and it trusts exactly
    the hooks purlis arms, so purlis's own hooks run and no other does.
  - **Network.** Its traffic, Codex's own requests included, leaves through purlis's proxy,
    which carries only the hosts of the project's egress presets. Codex checks certificates
    against the system's authorities.
  - **Linux.** purlis cannot wrap Codex on Linux yet (#1040), so there such a chat is still
    refused, with a sentence that says so.
- **New projects run their chats sandboxed.** A project that `purlis init` or the app makes
  now has `[sandbox]` with `mode = "on"` in its `charter.toml`. Its sandboxed chats can reach
  model providers, your forges and package registries (SD-2, #1056).
  - **Existing projects.** A project made before this keeps running as it did. The first time
    you open it, a notice offers to turn the sandbox on. "Turn the sandbox on" adds the same
    block to `charter.toml`, and "Keep it off" changes nothing. The notice does not come back
    either way.
  - **One chat without the sandbox.** The new-chat picker has "start without the sandbox", for
    that one chat, with a box for your reason. The chat's tab says it runs without the
    sandbox. A relaunched or resumed chat does not inherit the choice: it starts sandboxed, or
    not at all where the sandbox cannot be applied (a Codex chat, or a program the sandbox
    will not bind). Only a new chat can start without the sandbox; continuing an opted-out
    chat's conversation without it is not offered yet (#1098). If the sandbox cannot be
    applied on this machine, the picker shows why, and its button says "Start without the
    sandbox".
  - **Installing what is missing.** On Linux, if bubblewrap or socat is missing, the picker
    shows your distribution's install command. "Type it in a shell tab" opens a shell at the
    project root with the command typed. It needs sudo, so you run it yourself.
  - **Programs the sandbox will not bind.** A profile whose program is anywhere a chat can
    write or is named by a relative path, or a Claude Code profile whose program does not
    answer as Claude Code, is refused in a sandboxed project; the picker says why before you
    start it. It checks a profile's program only once you have approved the profile.
  - **Codex.** purlis cannot keep a Codex chat inside its sandbox yet (#1123), so in a
    sandboxed project the picker says so and a new Codex chat starts only without the sandbox.
  - **Windows.** There is no sandbox on Windows yet (#565), so chats there start without it
    and their tabs say so.
  - **Recorded.** Each start without the sandbox is a `trust.sandbox.off` event in purlis's
    event log on this machine. The sandbox coming back on for that chat is `trust.sandbox.on`.
    The chat's session record says `sandbox: off`.
  - **How often.** `purlis doctor` and Project settings show how many new chats in the project
    started without the sandbox on this machine. The count is never sent anywhere.
- **opencode chats run sandboxed on macOS.** In a project with `[sandbox]` `mode = "on"`, an
  opencode chat now starts instead of being refused. opencode has no sandbox of its own, so
  purlis runs the whole harness inside a profile it writes, which starts from "deny
  everything".
  - **Writes.** The chat can write its own directory and a temp directory of its own. Of
    opencode's own files, it can write only what a turn writes: its sessions and log. Its state
    is kept in the chat's own temp directory.
    It cannot write opencode's credentials, plugins or packages.
  - **Snapshots.** A sandboxed opencode chat makes no snapshots (#1075).
  - **Network.** Its traffic leaves through a proxy purlis runs for that chat, including
    opencode's own requests to its model provider. The proxy carries only the hosts of the
    project's egress presets.
  - **Keychain.** The profile also keeps the chat away from the keychain, so a project with a
    keyring vault can run sandboxed opencode chats.
  - **Hook reports.** A report purlis's hooks could not hand to the app is shown in the chat's
    window, not dropped.
  - **Linux.** purlis cannot wrap opencode on Linux yet (#1040), so there such a chat is still
    refused, with a sentence that says so (SD-2, #695).
- **⌘⇧F searches the content of your files** (Ctrl+Shift+F off a Mac, outside a chat). It opens a
  Search tab on the branch picked in the explorer, or the workspace or project in front, and you
  can widen it to the workspace, the project, or every open project. Literal or regular
  expression, match case and whole word. Hits stream in grouped by project, branch and file,
  with the matching lines and a count per file; a large result stops at a page and _Show more_
  carries on, and a new query stops the one before. ↓ steps from the box into the hits, ↑/↓ move
  through them, and Enter opens the hit in its branch's file tab at its line. The search never
  looks inside what git ignores, links, vaults, files named like a credential, binary files or
  files past the preview's 2 MiB. A file with a line longer than 256 KiB is listed as not
  searched. Search tabs are not reopened at the next launch (FM-8, #1111).
- Any chat can ask you for a credential it lacks, declared or not, with the `secret_ask` tool
  or `purlis secret ask <KEY> --purpose "<what for>"`, and every chat is told to at its start
  instead of asking you to paste one. The ask shows the chat's purpose in its own words, says
  when the persona does not declare the key, and goes to the persona's vault unless the chat
  names another. A key the vault holds already answers at once. The chat hears only filled
  (under the name you stored it as) or declined, never the value. The tool doesn't stop on
  Claude Code's permission prompt first, since you answer the ask in the window.
- **Install skills from a git repository in the Skill studio preview** (#2773).
  - **Add source** takes a repository's https:// URL, or a folder on this machine, and optionally its branch and the folder its skills are in. Browse lists the skills it holds.
  - **The install review leads with a summary**: the scripts a skill carries and what runs each, the network hosts its files name, its `!` commands, anything it writes outside the workspace, any text hidden from a reader, and any file that is not text. Risky items are marked, every file is one click away with hidden characters shown, and skills it refers to come along in the same review.
  - **An install is a copy, pinned** to the commit you reviewed, all or nothing, and one commit of the skill's files and `skills.lock`. A local install is kept in `skills.local.lock` on this machine.
  - **Updates are never applied by themselves.** A newer commit is offered with what its summary gains and loses, then each file's diff, your own edits beside it, and any skill it now needs; only Accept moves the pin. A source only the team's record names is not contacted from your machine until you confirm it.
  - **A private repository signs in with a vault entry** of your own: the token stays in the vault, is sent to that repository's host alone, and is never shown or written anywhere.
  - The same on the command line: `purlis skill install` prints the review and asks before it copies anything, and `purlis skill update` lists newer commits and shows each one's diff before it asks; `purlis skill update --confirm <url>` confirms a source.
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
- **The Inbox: everything in a project that waits on you, answered where you see it.** It is the
  first view on the right side's activity bar, and its icon counts what waits, while the side is
  away too. Each chat that asks is a group under its chain (`steward 12 › #3046 drill`), the one
  waiting longest first. A permission prompt, a choice of several, a sandbox host and a dispatch
  grant are answered in place, through the same path as their Notice, and answering in either
  place clears both. A chat waiting on your reply gets a reply box, a prompt in a harness's own
  terminal says so, and every ask has Go to chat, which opens a task in its session's tab. The
  title bar's ✋ opens it, and so does ⌘⇧I (Ctrl+Shift+I elsewhere). ↑ and ↓ move, Enter presses
  the focused button and Escape goes back to the chat; no single letter does anything. Empty, it
  says "Nothing is waiting on you" and lists what you answered there lately (#1692).
- **The Skill studio is ready to try** (Skill studio preview, #2786). Every skill your chats get,
  in one place: see what each chat has and why any other is left out, write a skill or ask a chat
  to draft one, install one from a git repository after reading what it adds, grade it with real
  chats against no skill and its previous version, keep a large library within each chat's skill
  budget, and publish or export a skill to share it. It is still a preview: turn it on in
  Settings › You › Previews with the _Skill studio_ switch, on each machine, and some of it may
  change before it is finished. [The Skill studio](docs/skill-studio.md) explains it from
  the start.
- **Themes at every level, and your choices kept per project and workspace.** Theme folders
  can live in your config folder's `themes/`, a project's `themes/` and a workspace's
  `themes/`. Your choice of theme and appearance is kept on your machine per project and per
  workspace, set in Settings › Theme, and survives a re-clone when the project
  has a `[project] id`; a project's `[theme]` and a LIVE workspace's settings commit a default,
  and your choice beats it. Switching workspace repaints with a short crossfade. An old
  single-file `theme.json` is offered, once, a move into a theme folder (#2323).
- **purlis estimates weekly users without an identifier.** The first update check of each
  ISO week reads the channel's weekly manifest, `latest-weekly.json` or `dev-weekly.json`, which
  has the same bytes as the manifest. Its download count is the estimate. The request is the
  same from every machine and carries no id. `DO_NOT_TRACK=1` opts out.
  `docs/updating.md` gives the estimator and its biases (OB-17, #688).
- **Merge and discard a task's own branch.** In its Changes tab, **Merge…** lands exactly the
  commit you were shown in the branch it was cut from, as a fast-forward or not at all, and
  **Discard branch…** removes its folder; each asks first. A merge that does not apply
  cleanly changes nothing and says why. Only you can have purlis merge or discard: no chat
  can, through purlis.
- **While you were away: one summary of what happened.** When you come back after five minutes
  or more away from the window (in another app or window, or with no key or pointer on it), one
  Notice per project says what happened meanwhile, each project its own: "While you were away: 7 tasks done, 1 failed, 2 waiting on you". Each part is a
  link: one chat goes straight to it, several open a list of them. A chat waiting on you is shown
  where it lives, where its question is answered as before, and a finished task goes to its row
  under the chat that asked; going to a failed task marks it looked at, as its needs-you item
  does. Only what happened while you were away is counted. The summary hides nothing: each task
  keeps its row and its Activity lines, and Dismiss puts away the summary and nothing else. _Away summary_ in Settings, under Chats list,
  turns it off (#1514).
- **A redesigned first window.** With no project open, the ways in now stay down the left of the
  window and each one opens its page beside them. Recent projects sit in the same place on every
  launch, and a person who has already been through setup is no longer counted through steps.
- **A local network log.** purlis's forge calls, through its own client or through `gh` and
  `glab`, `purlis report`'s filing and the updater's reads are listed in
  `~/.config/charter/network-log/`, a file a day, kept for 30 days. Each line says which feature
  called which host, with the method, the path with names masked, the status and the time it
  took, and whether it went to purlis or to a third party. It never holds a body, a header, a
  token, a query or the names in a path, and nothing sends it anywhere. Without an account, and
  unless you send a report, the only purlis addresses it lists are the updater's.
- **A project can say which purlis it needs.** When a newer purlis starts writing something an
  older one would get wrong, it lists the feature in `charter.toml`'s `requires` and sets
  `schema = 2`. A purlis that lacks the feature, or does not understand the project's `schema`,
  or cannot read its `charter.toml`, treats the project as read-only. `purlis` still reads it
  (`status`, `recall`, `statusline`, `doctor`, and the `list` commands of workspaces, personas,
  changes, sessions, harnesses and guards, plus `workspace recall`), saying once that it is
  read-only and why. Every command that could write it is refused, extension commands,
  `doctor --fix`, `init` and `reinit` included, with the version to upgrade to (update the app); `purlis
  doctor`'s `schema` row says the same. Keys and sections purlis does not know are kept when it
  rewrites `charter.toml` or a workspace's `workspace.json`. The window and the chat hooks will
  follow the same rule before any release carries it (#826).
- **A chat that starts in a repo gets a branch of its own.** Start a chat with *New tab in
  <repo>*, or after *Start new chats in <repo>*, and purlis cuts a new branch for it, in a folder
  beside the repo's clone, so two chats in one repo no longer edit the same files. The branch is
  named after the chat, or `chat-1`, `chat-2` and on. Clear *start on a new branch* in the picker
  to work on the branch the repo has checked out, as before. If the chat does not start, the
  branch goes with it. *New branch in <repo>…* on the repo's menu and in the palette cuts one
  without starting a chat, and new chats start on it.
- **A kill switch that stops every chat and shell purlis started.** Stop all on the title bar,
  or `purlis stop --all` in any terminal, interrupts and ends every chat's and shell's program in
  every project and every window within seconds. No chat starts again, not even from a relaunch,
  until you re-arm it from the title bar; you can still open a shell to look around. The stopped
  chats stay as tabs to reopen. Each stop, re-arm and tamper is one line in `kill-switch.jsonl`
  in purlis's config directory (ADR 0071).
- **A plane can run every Claude Code chat in a sandbox.** Add `[sandbox]` with `mode = "on"`
  to `charter.toml`, and every Claude Code chat purlis starts there runs inside Claude Code's
  own sandbox, which purlis compiles from one policy. It can reach only the hosts of the
  plane's egress presets. Claude Code's web tools are turned off, because the host list does
  not hold them. The chat can never read a vault's storage or write purlis's own state. A
  plane can turn the sandbox on but never off, and a mistyped `mode` turns it on. If the
  sandbox cannot be applied, the chat does not start, and purlis says what is missing and how
  to install it. `SECURITY.md` says what the sandbox covers and what it does not. Planes that
  say nothing run as before (#695).
  **What this cannot do yet:**
  - A plane with a keyring vault starts no Claude Code chat while the sandbox is on. Keyring is
    the default vault provider, so on most machines this release can sandbox only a plane with
    no keyring vault. That lifts when purlis can wrap the harness, or resolve secrets for the
    chat.
  - opencode chats are not started in a sandboxed plane yet.
- **A chat's commits are scanned for secrets and personal data before they are made, in any
  repository.** Every chat the app starts on a harness commits through purlis's own git
  hooks. Before each commit, including a merge commit, purlis reads the lines the commit adds.
  These refuse the commit:
  - a token or key in a vendor's known shape (purlis's forge-prefix rules plus gitleaks'
    prefix-anchored vendor rules);
  - an email address;
  - a card number;
  - a US Social Security number.
  git prints where each one is and what kind it is, with the value masked, and the chat joins
  the needs-you queue saying why. This holds in a workspace repo, a piece, or a repository
  outside any plane. The repository's own hooks (husky, pre-commit, git-lfs) still run after
  purlis's check, except `post-index-change` and `reference-transaction`, which purlis does
  not forward. The guard also refuses `git commit --no-verify`, `-n`, and a
  `core.hooksPath` or `GIT_CONFIG_*` set on the command line. These are not treated as anyone's
  email: documentation and private-use domains (`example.com`, `.test`, `.internal`, `.local`),
  no-reply addresses, `git@host` remotes, and image names like `logo@2x.png`. The allowlist
  below lets any other false positive through. Commits made by `cherry-pick`, `rebase` and
  `am` are not scanned (#592, ADR 0074).

### Changed

- **A `gh` or `glab` tool used with GitHub Enterprise Server or a self-managed GitLab needs
  `host`.** Without it the tool reaches `github.com` or `gitlab.com`, and a run naming your
  own host is refused with a sentence that says so: add `host` to the tool and allow the
  persona's tools again (#3869).
- **purlis is now licensed under the Apache License 2.0 (was MIT); copyright Purlis.** The
  licence and its NOTICE ship with the app and show under About › Licences. Releases up to and
  including v0.4.1 remain under MIT.
- **purlis's own skills are named `purlis-<name>` on every harness.** `handoff` is now `purlis-handoff`, and on Claude Code `/purlis:purlis-handoff`. Old names keep working where purlis reads them: `purlis:handoff` in a persona's `skills:` list, a skill's text or `skill_load` reads as the new name, so no project has to change. A bare `handoff` is now an ordinary skill name, so a project's skill named `handoff` loads. A bare `/smart-close` no longer starts purlis's smart close (#4524).
- **Certificate checks are on by default in every sandboxed chat.** `gh`, `glab` and other Go
  tools verify a host's certificate through a macOS system service, and a sandboxed chat was
  refused it until each person allowed it on each machine, so those tools failed with
  `x509: OSStatus -26276`, or with an error that hid the cause (`gh auth status` said the token
  was invalid). Now every sandboxed chat has it, on Claude Code, Codex and opencode, with
  nothing to allow:
  - Settings › Sandbox has one switch, **Certificate checks**, on unless the project turns it
    off. The separate "on this machine" row is gone;
  - `[sandbox] certificate-checks = false` turns it off for everyone in a project. `true` says
    what leaving the key out says. **A project that never set the key now has the check**;
    nothing in your files changes;
  - an administrator's `sandbox.socket-grants: false` turns it off on that machine, and
    Settings says who set it;
  - a chat that was already running when you updated keeps the sandbox it started with: the
    Sandbox changed Notice lists it with Restart, and a block in it says the same;
  - a chat that has the check and whose tool still fails a certificate check is told so, and
    is not told to restart.
  What this leaves open, by decision: that system service makes its own requests, for revoked
  certificates and for the certificates a certificate names, and those do not go through the
  sandbox's proxy. Turn the switch off in a project where that matters more than those tools.
- **Chats run in the session host by default.** Every chat now runs in purlis's session host
  rather than in the app's own process, so a crash of the app no longer ends it. Nothing you see
  or do with a chat changes. Upgrading loses nothing: each chat that was open reopens on its own
  conversation, on its own profile, in its own folder and branch, and your session records and
  the profiles you declared yourself work as they did, with nothing to set up or approve again.
  Opening a project writes nothing into it. Quitting still ends every chat, and so does an
  update. If chats misbehave in the session host, you can run them in the app again while purlis
  still carries both: on macOS run `launchctl setenv PURLIS_CHATS_IN_THE_HOST 0` in a terminal,
  then quit purlis and start it again (`launchctl unsetenv PURLIS_CHATS_IN_THE_HOST` and a
  restart undo it); on Linux start purlis with `PURLIS_CHATS_IN_THE_HOST=0` set. Switched back
  after a crash, purlis first ends the chats the session host kept, as a quit would, and then
  reopens them in the app, so no conversation runs twice. Tell us what made you switch back
  (#2004).
- **Claude Code chats now go through purlis's proxy, as Codex and opencode chats do.** Each
  Claude Code chat gets its own pair of proxy ports, HTTP and SOCKS5, and purlis names both in
  the settings it starts the chat with. Every host the chat reached before is reached the same
  way, and every connection is in the network record and on the chat's Network view. This needs
  Claude Code 2.1.285 or later. An older Claude Code keeps its own proxy and the same allowed
  hosts, and says so once.
- **A Claude Code chat can no longer connect to other local ports or listen on one**, even where
  your own Claude Code settings turn local binding on, so it reaches the network only through
  its own two ports. Codex and opencode chats already could not.
- **Closing never ends a chat.** Close pane clears a pane and Close tab closes a tab. Their chats
  keep running in the sidebar, nothing is asked, and the close button no longer turns red under
  the pointer. To end a chat, use End… in its list. Closing the last tab on a strip leaves one
  empty pane (#4756). Undo, ⌘Z or the palette's row, puts a closed pane or tab back as it was
  (#4763).
- **New keyring items are stored under `purlis/`, and items under `charter/` are still read.** A
  new keyring vault's items, and a newly stored 1Password identity, are written under `purlis/…`.
  A vault or identity stored before the rename keeps working from its `charter/…` item, and the
  check that an item is charter's own accepts exactly those two prefixes. 1Password items charter
  writes are tagged `purlis` (RN-4, #1261).
- **One list: the Inbox holds what the Alerts drawer and the band under the tab strip held.**
  Every line that stood under the tab strip, two at a time, is now a Notice in the project's
  Inbox, after what asks for you and before what happened, every one listed with its ways out.
  The alerts are Notices there too: this project's and this machine's whole, and one line for
  each other open project that has some, which opens that project's Inbox. The status line's
  Alerts button is its Notices button, counting them and opening the Inbox. A Notice that
  answers what you just did (a refusal, an Undo, a saved record) or the summary of a time away
  brings the Inbox on screen (#1695).
- **Work a chat needs an answer from has one route, `purlis dispatch`.** A chat told to
  dispatch to a persona could run a reporting handoff instead, because the handoff skill
  taught `--report` as the way to get an answer and `--persona` for another persona. The
  handoff skill, the persona skill, `purlis docs show handoff`, `purlis handoff --help` and
  the briefing of a chat the app started (as where such work goes, not as a reason to give
  work away) now say one thing in the same words: work a chat
  needs an answer from, for its own persona or another, is `purlis dispatch` (with `--in
  workspace:<name>` when it must run elsewhere), and a handoff is fire-and-forget. The
  refusals that named `purlis handoff <workspace> --persona <name>` as the way to dispatch to
  a persona (a vault a chat may not use, `purlis persona use` inside a chat) name `purlis
  dispatch --to <name>` (#1515).
- **The command line is `purlis`, and `charter` still runs it.** The app ships `purlis` beside
  itself with `charter` as an alias for the rename's window, and **Install `charter` command in
  PATH** now links both names; the `.deb` installs both. Every guard that recognises charter's
  own command recognises `purlis` too (RN-3, #1255).
- **The product's environment variables are now `PURLIS_*`, and `CHARTER_*` still works.** Every
  variable that steers a command, set by you, by the app for a chat, or by a harness
  (`PURLIS_ROOT`, `PURLIS_HOME`, `PURLIS_WORKSPACE`, `PURLIS_CONFIG_HOME`, `PURLIS_DATA_HOME`,
  `PURLIS_LOG_DIR` and the rest), is read under its `PURLIS_` name first. Its `CHARTER_` name is
  read when the `PURLIS_` one is not set. Chats and extensions are given both names until 1.0,
  so hooks and scripts that read the old name keep working. A harness profile may set neither.
  A variable that chooses what a command acts on is refused rather than guessed when both names
  are set to different values. Those variables are the project, its state, the workspace, the
  worktrees, the persona, and the config and data homes. The refusal says to set both
  names to the same value (`PURLIS_ROOT=<x> CHARTER_ROOT=<x> charter …`). So a single name
  typed in front of a command inside a chat tells you what to type, instead of acting on the
  chat's own project (RN-2d, #1260).
- **Purlis is the default theme.** A new install, and any project, workspace or machine where
  nobody chose a theme, now draws Purlis: the lavender dark from the designer's foundation, in
  IBM Plex Sans and JetBrains Mono with Lucide's icons. When your system asks for more contrast,
  it draws Purlis's high-contrast dark. A dark, light or "Follow the system" pick now draws
  Purlis too: `purlis-dark` (and `charter-dark`) draw Purlis's dark, `purlis-light` (and
  `charter-light`) its light, and `system` follows the system with your own `theme.json`, or
  else with Purlis. No file is rewritten. Settings › Theme no longer offers `purlis-dark` and
  `purlis-light`, and labels a file that still holds one. To keep the green look purlis had
  until now, choose **Purlis Classic** (`purlis-classic`). Purlis is no longer marked Preview,
  and a new theme draft extends Purlis (#3034).
- **A sandboxed Claude Code chat takes its settings from purlis and from you, not from files
  in the project.** At each start of a sandboxed Claude Code chat, full sandbox or floor, purlis
  reads the project's `.claude/settings.json` and `.claude/settings.local.json`. They are loaded
  as they are while they hold only deny and ask rules, plugins, model, output style, theme,
  and the entries purlis's own `init` writes.
  If they hold anything else, or cannot be read, the chat starts on your own Claude Code
  settings and purlis's. The project's deny and ask rules still apply. The chat's tab names
  the keys that were not applied, and says that the project's CLAUDE.md, rules, skills, agents
  and commands are not loaded in that chat either. A chat with no sandbox is unchanged.
  Commands and the Edit tool in a sandboxed chat do not write your own Claude Code settings
  files, at the floor or with the sandbox fully on.
- **Search and Changes are views on the left, and the bottom panel is gone.** ⌘⇧F (Ctrl+Shift+F
  elsewhere) shows the Search view on the left side instead of opening a Search tab, searching
  the branch, workspace or project you are on and keeping what you typed. ⌃⇧G shows the Changes
  view: each repo's branch, uncommitted files, branches and pipeline, which the bottom panel
  showed. Its icon counts the uncommitted files, also while the side is put away. The terminals
  now have the window's whole height, and the status line has two toggles, Navigation and
  Attention. A layout file that still places the bottom panel is read without it (#1676).
- **The app is purlis now: `purlis.app`, bundle identifier `dev.purlis.app`.** At its first launch
  the app moves its log folder from `dev.charter.app` to `dev.purlis.app` before it writes a log
  there, and holds its keychain items again under the new identity without asking: a vault whose
  items macOS would ask about keeps working and waits for **Finish moving**, which asks once per
  secret. Nothing moves while another charter is running, and purlis does not start while the
  old app still runs. macOS treats the renamed app as a new one, so it asks again for permission
  to show notifications. An install that updated itself keeps the folder name `charter.app`; a
  fresh `purlis.app` replaces it, and the old `charter.app` can be deleted. On Linux the
  one-per-user lock and the D-Bus name follow the new identifier, and the `.deb` is the `purlis`
  package, which replaces the `charter` package. Updates, the release assets
  (`purlis-macos-arm64.dmg`, `purlis-linux-x86_64.deb`, …), About's links and `charter report`
  use `purlis/purlis`; installed builds still find new releases through GitHub's redirect from the
  old address (RN-9, #1267).
- **Every chat now runs at least at the floor** (#2071). In a project that has not turned the
  sandbox on, a chat reaches every host and writes every folder but what the floor holds, so
  it works much as its harness does unsandboxed. It still cannot read a vault or purlis's own keys, cannot write
  what a later program runs (git hooks and config, `.claude` settings, editor tasks), and
  cannot write a folder on your `PATH` or your harness's own program; a refused install raises
  the usual block Notice. A project with `[sandbox] mode = "on"` keeps the full sandbox. Each
  chat's tab shows its level: full sandbox, floor, or no sandbox.
- **A local project in the config home moves to the data home at the next launch.** It moves in
  one rename, with a link left at the old place until its clones' worktrees, the chats it
  reopens and the projects this machine remembers all name the new one, so nothing is lost and
  its approval and pins go with it. A launch that stops halfway is finished by the next one. One
  that something still works in (a terminal, an editor, another purlis) is left where it is: the
  app's log and the project's doctor say so, and the move is made at a launch when nothing works
  in it. A worktree outside the project keeps the link, and the log names the
  `git worktree repair` to run (#1670).
- **A Claude Code conversation from a chat that ran in the old place does not come back by
  Resume.** Claude Code keeps a conversation under the folder its chat ran in, and that folder
  has a new name. The conversation's file is not touched (it stays in `~/.claude/projects/`);
  Resume starts the chat fresh with its session record in its briefing, as for any conversation
  the harness cannot find (#1670).
- **The window's forge calls use your purlis sign-in, so `gh` and `glab` are optional.** Once
  you are signed in to a host in Settings › You › Accounts, the workspace repo picker, the
  Doctor's discover, the save button and the workspace's LIVE switch (and the pull or merge
  request they open), and the workspace panel's request and CI column ask that forge over HTTPS
  with your sign-in, read from the keychain for each request. A host with no purlis account
  still uses `gh` or `glab` when they are installed; when they are not, the message says to
  sign in under Settings › You › Accounts. Chats, `purlis` commands and saves nobody pressed for
  never use the sign-in (#1816).
- **purlis runs far fewer git processes in your repos while it is idle.** Auto-save, the
  title bar's save indicator, the Saving tab and the alerts used to each run `git status` and
  its friends on their own timers — about ten git processes every two seconds per project, and
  more in a busy repo, where they could pile up on each other. They now share one reading per
  repo, made again only when the repo's git files move, its project's watcher sees a change, a
  save or fetch finishes, a chat ends, or ten seconds have passed. Only one git process runs in
  a repo at a time for them. Where your git config says nothing about `core.untrackedCache`,
  purlis turns git's untracked cache on for these reads and for its saves, so `status` stays
  quick in a large repo: the cache is then kept in that project's and those clones' index, as
  git keeps it (`git update-index --no-untracked-cache` removes it, and setting
  `core.untrackedCache` yourself, to anything, stops purlis asking for it).

### Deprecated

- **The charter names stop working at 1.0.** Until then purlis reads its old names and writes
  only the new ones. From 1.0 it stops reading them: the `charter` command (run `purlis`), the
  `CHARTER_*` environment variables (set `PURLIS_*`), a project's committed files under their
  charter names (`purlis doctor --fix rename-plane` moves them), this machine's local state under
  its charter names, such as `.charter/`, `charter.local.toml` and the config and data homes
  (`purlis migrate` moves them), and keychain items under `charter/` (the same migration copies
  them to `purlis/`). The removal waits past 1.0 while `purlis doctor` still sees an old name in
  a project you opened, so run it before you update: it names every old name it finds. A
  migration waits until nothing named charter or purlis is running, and going back to a build
  from before the rename on the same machine needs `purlis migrate --undo` first. A project
  `rename-plane` moved is no project at all to a teammate on a build from before the
  compatibility check, so update every teammate before you move a shared project. Every
  version's notes say this until 1.0 (ADR 0091).

### Removed

- **Settings › Chats list › Rows.** A row is always one line. A layout file that still says
  `chats.lines` is read as if it did not (#1675).
- **A pressed task no longer becomes an ordinary tab.** It did, for good, and its close button
  read as ending the session. A task whose session has no tab in the window (the session was
  closed and left it running) still opens in a tab of its own. The "no tab" badge on a task's
  row is gone: no task has one (#1486).
- **`purlis browser install` is retired.** A chat purlis starts drives a real browser with
  purlis's own browser tools, which the `browser` skill teaches, so purlis no longer generates
  Playwright's page-driving pages into a project. Running the command writes nothing and says
  what to use instead. Pages it generated before stay where they are (#2575).
- **The separate Project settings page.** Its forms are the Project level's groups in Settings,
  and its Raw TOML view is Edit as TOML. Project settings… on a project's menu and in the
  palette opens Settings at the Project level. A Project settings tab left open by an earlier
  version comes back as that too (SE-19, #1169).
- **The two icons at the bottom left that put a side away.** They did what each side's own
  strip of icons already does: press the open view to put the side away, and any view to bring
  it back. ⌘B and ⌥⌘B (Ctrl+B and Ctrl+Alt+B off a Mac) and the palette's rows do the same.
- **Persona sub-agents are retired. After updating, run `purlis doctor --fix persona-agents` in
  each project, then save.** purlis no longer generates `.claude/agents/<persona>.md`, and
  `purlis persona sync-agents` (with its `--approve-mcp`) is gone: the word is still recognised
  and answers with one sentence that says what replaced it. The fix removes the files purlis
  generated, where git tracks them and they have no uncommitted change, and leaves and names
  every other file. It prints everything it changed, makes no commit, runs only by name, and
  changes nothing on a second run. Until it is run, Claude Code still lists each generated
  file as an agent type; a call to one is refused, and each chat is told so at its start. A
  project's own `CLAUDE.md`, README, charters and scripts may still say `sync-agents` or
  `subagent_type`: search for both (#1451).
- **A persona whose `model:` named a model now runs on the asking chat's profile.** `model:
  opus`, `sonnet` or `haiku` picked the generated sub-agent's model, and nothing reads it now,
  so that persona's chats run on the model of the chat that dispatched them, which may cost
  more or less. Name the profile a persona's chats start on with `profile:`. The fix and
  `purlis persona lint` say this for each persona it applies to. A `model:` that is the name of
  a profile the project carries is rewritten to `profile:` (#1451).
- **`agent-tools:`, `skills:` and `memory:` are read by nothing.** A
  persona chat has every tool of its harness, so a persona that listed no editing tool could
  not edit files as a sub-agent and can as a chat. Nothing asks before two persona chats write
  in one tree; `dispatch-isolation: worktree` now gives a dispatched chat a worktree of its own
  instead (#1453). `purlis persona lint` and the fix say what widened for each persona and leave
  the lines. To keep a persona from a tool, name it in `disallowed-tools:`. The hooks no longer
  log a returned sub-agent or a message to one in the dispatch log (#1451).
- **purlis no longer turns other harnesses' plugins on or off per project.** A plugin brings
  hooks, MCP servers, subagents and code as well as skills, and each harness handles those its
  own way, so which plugins load is now the harness's own settings' to say. The Plugins group in
  Settings, the plugin rows in the Skill studio and `[harness_plugins]` are gone. purlis's own
  plugin loads as before on all three harnesses, and `purlis plugin install` stays. The Skill
  studio still lists the skills a plugin brings, read-only, naming the plugin (ADR 0050, #3467).
- **The old overlay and menu classes are gone.** `.ui-menu`, `.ui-menu-item`, `.ui-popover` and
  `.ui-overlay-arrow`, which nothing in the window had worn since the menu, popover and tooltip
  families replaced them, are no longer drawn. A theme that targeted them reached nothing already;
  the families' parts are what a theme reaches (#3365).
- **Search no longer has a view tab.** The left side's Search view is the only place a search
  is shown (#1701).
- **The explorer no longer lists the chats a chat started in other workspaces under its row.**
  The Chats list is the one tree of who asked whom (#1490).

### Fixed

- **A slow machine or runner no longer fails a chat's start.** The dialog says "Starting the session
  host…" for as long as it takes, Cancel works throughout, and it shows an error only when the host
  has actually failed. A chat that comes up after you cancel is closed.
- **A clone and a removal at the same moment no longer lose a workspace's repo.** Cloning a repo
  into a workspace and removing another from it both rewrite the workspace's `workspace.json`.
  When the two ran at once, from the window and a terminal for example, the later write could put
  back the list the earlier one had changed, so a repo it had just recorded or removed came back
  or went missing. Each now waits for the other (#1249).
- **Two writes to one keyring vault never lose each other's secret.** A `secret set`, a
  `secret delete` and the move of an item under purlis's access rule now take the vault's own
  lock, so a write that lands while another is in progress, or while an item is being moved,
  waits for it instead of being overwritten with an older value. The wait is bounded: a writer
  still held up after ten seconds is refused and writes nothing (#1180).
- **A paste in a chat no longer quits purlis on macOS.** When macOS raised an error of its own
  while it handled a key (seen on Cmd+V under macOS 27), purlis quit with "purlis quit
  unexpectedly". macOS now reports the error and purlis keeps running. A fault in purlis itself
  still ends the app, as before.
- **Quitting while a launch is still putting chats back keeps every chat** (#3114). A chat whose
  start finished just as purlis quit could drop out of the record of open chats, and the next
  launch did not put it back. It is now kept as a chat still to start, in its place on the strip
  and in front if it was.
- **A request-mode save no longer blocks for good after the plane's history is rewritten.** When
  the save branch on the remote holds only commits the plane already has under new shas (the same
  changes, rewritten with new dates), the save pushes over it instead of saying it "has commits
  this clone did not push there". A commit on it that the plane does not have still blocks the
  save.
- **A message to or from a dispatched task is no longer lost when the app does not answer.**
  `tell`, `note`, `ask` and `answer`, and their tools, are queued and delivered, in order and
  once, when the app next hears from the chat; the sender is told the message is queued, not
  yet delivered, and `purlis dispatch list` shows each message as queued, delivered or
  undeliverable, with why (#3902).
- **Plain-file and reference vaults no longer look empty after your project's state folder
  moves.** A vault records its file as `.charter/vaults/<name>.json`, and moving the state folder
  to `.purlis/` left that record pointing at nothing, so the vault read as empty and the next
  `secret set` could start a second file. A vault file under the state folder is now read and
  written in the folder the project has, under either name, with nothing to rewrite. A vault file
  that is missing after values were written to it is now refused with its path, instead of being
  read as an empty vault (#1285).
- **A project whose chats hang as they start no longer holds up every other project's chats at a
  launch.** One project's chats take all the places a launch starts chats in but one, so the
  other projects' chats come back at once instead of one project at a time, minutes apart. A
  launch now starts three or four chats at once, by the machine's cores (#4986).
- **An older runner's chats' conversations are no longer lost when it had already stopped.** After
  an update or a Replace, a runner that had stopped first, for a suspend or because its container
  was removed outside purlis, was removed with its home and every conversation its chats left
  there. Now purlis looks in that home first: where it holds a conversation, the runner is kept,
  and Devices offers Bring back conversations, which puts them in the pool's runner's home where
  their harness resumes them and then removes the old runner, or Discard…, which asks twice. A home with no conversation goes as before.
  opencode's conversations are kept but not brought back. Replace no longer keeps a runner already
  stopped whose home holds none.
- **purlis no longer makes macOS ask whether it may "access data from other apps" for a
  1Password vault read through a service-account token.** The 1Password CLI read the
  1Password app's settings from that app's container on every run; purlis now tells it not
  to when the run signs in with a service-account token (#1654).
- **Typing into a chat no longer freezes the window while its session host is slow.** A pane's
  keys, paste, mouse, resizes and its view going on and off screen used to wait for the session
  host's answer before the window could draw again, for up to 30 seconds on a host that was busy
  or coming up. Each chat now has a queue of its own: what you send goes in at once, reaches the
  chat in the order you sent it, and other chats and the window carry on meanwhile. When it
  cannot reach the chat, because its runner is reconnecting or the host refused it, the chat's
  pane says so with the reason (#4734).

### Security

- **After purlis fills a secret into a page, that browser reaches only the page's site until the
  page is left or closed.** A route change inside the page keeps the hold, and the filled field
  can't be copied, cut or dragged out meanwhile. Going back to the page brings it back empty
  (#5117).
- **If you run two versions of purlis on one project:** a version from before this change that
  rewrites this machine's record (`app/sandbox.json`) drops the grants you limited to a
  workspace and what you accepted of the project's. Nothing widens: those dispatches ask
  again (#1505).
- **A Docker pool runner made before this version keeps working, but its chats' servers do not
  listen** (unix sockets included) until you remove the runner and add it again. A Kubernetes
  runner's chats listen nowhere yet (#3413).
- **A task never starts with its permission prompts off.** A chat running with them off starts
  its tasks in auto mode, where Claude Code's classifier still judges every tool call, and
  purlis's sandbox and guards still apply (#3787).
- **The git purlis runs starts no program a repository's own config names.** A repository's
  config can name a filter, a diff or merge driver, a signer, an editor, a trailer's command, or
  what a remote starts at the other end. purlis's own git turned off only the repository's hooks
  and fsmonitor. Now it turns off every one of these that a repository sets. Your own global and
  system config is not affected, and the standard Git LFS filter still runs. Where turning a
  program off cannot leave the command working, purlis stops instead: a merge that needs the
  repository's merge driver stops on a conflict, a signed save in a repository that names its own
  signer is refused, and a fetch or push in a repository that names a remote's program is not
  run (#1805).
- **A vault's token is given to purlis itself and is kept in the system keyring alone.** A
  vault set up this way binds no environment variable, so there is no export for a chat's
  shell to read. The keyring record now also pins the item a vault keeps its secrets in.
  `vault add --token-stdin` refuses inside a chat before it reads anything (#1527, ADR 0047's
  2026-10-09 amendment).
- **An administrator's policy locks sandbox values, and the strictest wins.** purlis reads this
  machine's `/etc/purlis/policy.json`, only where root owns it and its folder and no one else can
  write either (never a link), and no chat may write its folder. It can fix the Internet access
  presets and the hosts any level may add (a project's forges included), and forbid your own
  hosts, a persona's own hosts, a person's opt-out, and folders granted to a chat. A policy that
  forbids the opt-out requires the sandbox in every project on that machine. Each locked value
  shows "Locked by policy" and who set it in Settings › Sandbox, with no control; the new-chat
  picker, a block's Notice and a held persona's Notice offer nothing policy forbids and still
  say why and who to ask. A policy file, or its folder, that purlis cannot trust or read locks
  everything it could lock but the presets (#1343).
- **A sandboxed chat can no longer author or edit a project's skills at all.** In every
  harness, a sandboxed chat may never write `.claude/skills/`, `.agents/`, or opencode's
  `tui.json` and `tui.jsonc`, at any depth. These join the other harness config it already may
  not write. Each of them can make a later chat, run outside the sandbox, start code (#1057).
- **A sandboxed Claude Code chat can no longer read or write what waits to be told to another
  chat.** A dispatched chat's report, and purlis's own word on a dispatch you were asked
  about, wait in the project's state folder until the chat they are for takes its next turn.
  That folder is now denied, for reading and writing, to a sandboxed chat of a harness whose
  hooks run outside its sandbox: a chat could have written a line there that another chat
  would have read in purlis's voice. purlis's hooks deliver, outside the sandbox.
- **On Codex and opencode that folder is still readable and writable by a chat.** Their
  sandbox is a wrap around the whole harness, so the hooks that deliver run inside it and
  must reach the folder. What holds there is the check each file gets as it is read. The
  denial reaches them once delivery moves onto the hook socket (#1457).
- **An unattended agent cannot print the forge token or read other stored secrets.** The CLIs'
  token commands, the auth-status and config reads that show the token, git's credential fill,
  and commands that read the OS keychain or a password store are refused when nobody is
  watching, since a run holding a secret could act where no guard sees it. Attended use is
  untouched (#866).
- **A chat starts from an allowlisted environment.** A chat is no longer given the app's whole
  environment minus a few names: it gets a keep-list (`PATH`, `HOME`, the locale, the proxies,
  `XDG_*`, `CHARTER_*` and the like), the variables its harness declares, and whatever the plane's
  `charter.local.toml` lists under `[chat_env] pass`. Cloud, forge and model-provider credentials
  pass only when listed by exact name.
- **Hook calls are authenticated per chat.** Each chat gets its own token when it starts, and the
  hook channel accepts a call only with that chat's token.

## [0.4.1] - 2026-09-28

0.4.1 is a chat that ends well and a memory you can look after from the window. Closing a chat
offers **Smart close**: the chat writes a session record of what it did and what is left, goes
into the background as a chip while it does, and its tab closes once the record is saved. Each
workspace, and the plane root, lists its records in a **Sessions** panel, and **Resume** brings
one back as a new chat on its conversation, harness, profile and directory, with the record in
its briefing. A memory opens in a tab of its own, rendered as Markdown, to be edited in place or
deleted with an Undo, and the right column lists a workspace's memory and the shared memory,
with a `+` for a new one. Beyond the window: `purlis session record|list|show`, command-line
verbs that edit, archive and unarchive a memory, and Codex and opencode chats that come back to
their own conversation at a relaunch.

### Added

- **Smart close.** Closing a chat now asks **Cancel**, **Close** or **Smart close**. Smart close
  sends the chat one line asking purlis's `smart-close` skill to write its session record — at
  once to a chat that is waiting, at the end of its turn to one that is running — and the tab
  closes only when `purlis session record` says the record is saved. Meanwhile the chat goes
  into the background: its tab shrinks to a chip — the chat's icon and a breathing amber mark,
  its name in the tooltip — at the left edge of the chat strip, before the pinned tabs, and the
  chat that was beside it comes to the front, as Close would bring it. Click the chip to watch
  the chat work; it stays a chip until it is done, and it cannot be dragged. The chip's menu
  offers **Cancel smart close**, and typing into the chat cancels it too, except to answer a
  question the chat asks on the way. When the record is saved the chip goes, and a quiet notice
  says **Session saved — <title>** with **Open record**. If no record arrives within five
  minutes, the chat ends first, or its prompt could not be sent or Smart close could not start,
  the tab comes back where it was, the window says so in a sentence, and the title bar's ✋ list
  names the chat and why. In a Claude Code chat the app starts, `purlis session record` runs
  without asking permission — that command and no other, and only as a command of its own: your
  own and the project's permission rules all still apply beside it. Codex runs it without asking
  in its default sandbox, which keeps the command from reaching the app, so the command leaves
  the news beside the chat and the chat's own `Stop` hook, which runs outside the sandbox, passes
  it on at the end of the same turn — only for that chat and conversation, and only within the
  five minutes the app waits (#517). Smart close is not offered on a shell tab, a chat never
  prompted, a chat purlis has heard nothing from, or one asking you something — answer it
  first. **Close** is the default for a chat that has had at most one turn (ADR 0064, ADR 0039).
- **Session records.** `purlis session record --title "…"`, with the record on standard input,
  files a summary of a chat's session — Goal, Done, Decisions, Open, How to resume — in its
  workspace's `sessions/` (the plane's own `sessions/` at the plane root), keeps
  `sessions/index.md` newest first and a `## Sessions` line in `workspace.md` pointing at it,
  and tells the app the chat's record is saved. Which chat, persona, harness, harness profile,
  directory, conversation and pieces a record is about is purlis's to say, never the model's.
  `purlis session list` and `purlis session show` read them back, a chat's briefing names the
  last one, and purlis's new `smart-close` skill writes one. A new workspace's `workspace.md`
  has a `## Sessions` section (ADR 0064).
- **Old sessions come back from the window.** Each workspace has a **Sessions** panel — and the
  plane root's tab one of the plane's own — listing its session records newest first, with
  when, persona and harness, and `↻ resumable` on a record that holds a conversation. A row
  opens the record as a read-only tab. **Resume**, on a row's menu and on the record's tab,
  starts a new chat in the record's place, on its harness profile and in its directory — in a
  piece, for a record written from one — given its conversation back (`claude --resume`,
  `codex resume`, `opencode -s`, as a relaunch does) and as its persona where the plane still
  has it, with the record quoted in its briefing. Where the profile or the directory is gone, or
  the record is older than them, Resume uses the project's default profile or the workspace's
  directory and says so. Where the conversation cannot be given — the record holds none, names
  no harness a profile here runs, or the harness can no longer find it — the chat starts fresh
  with the record in its briefing, and says why. The palette has **Open session record: …** and
  **Resume session: …** for the place in front.
- **A memory opens in a tab of its own, and can be edited and deleted from the window.** A
  memory row opens the memory as a preview tab — in italics, and replaced by the next memory you
  click, as VS Code's preview is; a double-click on the tab, or starting an edit, keeps it. The
  tab says which store the memory is in, when it was written and which file it is, and renders
  its body as Markdown. **Edit** turns the same tab into a title field and the raw body; a save
  over a change made on disk since you opened it is refused, with **Reload** and **Overwrite**.
  **Delete** moves the memory to its store's archive and closes its tab, with an **Undo** for a
  few seconds. A row's menu offers Open, Edit and Delete, and a list's search now matches a
  memory's body too, with the matching words shown under the row. Each acts on the file it
  names and no other: `deploy` never reaches `prod-deploy.md` (ADR 0065).
- **A workspace's memory, and the shared memory, in the window — and a new memory from a `+`.**
  The right column has a **Memory** section directly under Todos: the focused workspace's
  memories, newest first, with the same search and the same Open, Edit and Delete as a
  persona's memories. It is not shown at the plane root, which keeps no memory of its own. The
  Personas section ends with a **shared** row saying how many memories every persona shares;
  it opens them as a list in a tab of its own. A `+` on the Memory section, on a persona's tab
  and on the shared list opens a new memory in edit mode, and Save lists it where it was made.
  The palette offers the same: New memory in, for or shared, and Open shared memory (ADR 0065).
- **Edit, archive and unarchive a memory from the command line.** `purlis workspace
  edit|archive|unarchive` and `purlis persona edit-memory|archive-memory|unarchive-memory
  [--shared]` do what the window does. Edit takes `--title`, and the body as an argument or as
  `-` to read standard input; a slug is the memory's exact name, or the one file whose name
  ends `-<slug>.md`; and an archive that had to number its file prints an Undo that restores
  the original name with `--as` (ADR 0065).

### Changed

- **A short slug that more than one file ends in is refused, and the refusal names them.**
  `purlis persona forget` and `purlis ws todo done|forget` took the first in sorted order.
  `MEMORY`, the index, is refused as a slug, on a case-insensitive filesystem too.
- **A LIVE workspace publishes its session records** with its memory and todos: the managed
  `.gitignore` block un-ignores `sessions/`, and `purlis reinit` brings an older block up to
  date. **The plane root's session records stay on this machine**: `purlis init` writes, and
  `purlis reinit` adds, `/sessions/` to the plane's `.gitignore`.

### Fixed

- **A memory another memory's title mentions comes back to the index when it is unarchived.** A
  title holding `(b.md)` counted as listing `b.md`, so unarchiving `b` added no index line for
  it. The index now lists only the file each line links, as editing and deleting read it. And
  `optimize --apply`'s index repair can no longer lose a line to an edit saved at the same
  moment (ADR 0065).
- **An older workspace's `notes.md` is indexed even when a memory's title mentions it.** A
  title holding `(notes.md)` stood in for the memo's own index line, so the memo stayed
  unindexed; and that line is now added under the same lock as every other, so an edit saved
  at the same moment cannot drop it.
- **The title bar's look at a plane no longer rewrites git's index.** Asking which files are
  unmerged ran `git diff`, which refreshes `.git/index` under `index.lock` whenever a tracked
  file has been touched without changing — so a save starting at that moment could fail on the
  lock. It now asks `git diff-files`, which only reads.
- **Codex and opencode chats come back to their conversation at a relaunch, and a Claude Code
  chat comes back to the one it was in after `/clear`.** A chat's conversation was recorded
  only when it started, which only Claude Code has, so every Codex and opencode chat came back
  as a new one, and a cleared Claude Code chat came back to the conversation it had cleared
  away. The id the chat's own harness reports through its hook is now written into
  `.charter/app/reopen.json` as it arrives. A harness started inside the chat's shell still
  moves nothing (ADR 0024).
- **Forgetting a memory leaves every other memory's index line alone.** A line of `MEMORY.md`
  whose title mentioned the forgotten memory's file was dropped with it. Only the line whose
  own link names the file goes now, and a title's `\`, `[` and `]` are escaped in the index so
  a title can never read as the end of its own link.

## [0.4.0] - 2026-09-27

0.4.0 is the IDE growing up. The plane root is a tab of its own, first on the workspace strip,
and a **Curate ▸** menu opens a chat for a curation action, in Claude Code or Codex, with its
prompt typed and never sent. Shell tabs are plain shells that warn when a harness is started in
one by hand, every tab strip can be reordered by dragging, a chat's pane has find and takes
Shift+Enter as a new line, and a trackpad scrolls it the way a native terminal does. Vaults,
personas and todos can be created and deleted in the window, purlis's skills reach Codex and
opencode chats, and the window's own chrome no longer selects as text. Beyond the window:
cross-repo changes with `purlis change`, `purlis report` for filing issues, workspace rename,
projects in windows of their own, and opencode chats started by the app.

### Added

- **The plane root is the first tab of the workspace strip.** It is always there, drawn as an
  icon (its tooltip: *Plane — chats here start at the plane root*), and it cannot be dragged,
  pinned, renamed or deleted. Its menu starts a chat or a shell at the plane root, and so do
  `New tab` and `New shell` while it is focused. A chat started there is in no workspace on
  purpose: it is not asked which workspace to use, its briefing lists the plane's workspaces as
  ones it may manage, and `charter` asks it to name one with `-w` rather than acting on one it
  picked (`purlis workspace use` does not move it). The Todos box is not offered there, and
  the region says why. It replaces the "Outside every workspace" tab, and the chats it held are
  on it.
- **Anywhere in the plane outside every workspace is the plane root, and says so.** A chat
  started in `docs/`, `.charter/` or any other directory of the plane that is not a
  workspace's is marked as a plane-root chat, like one started in the plane's own directory,
  and is not asked which workspace to use. A terminal session standing there that has not
  chosen a workspace (no `-w`, no `$CHARTER_WORKSPACE`, no `purlis workspace use`) is at the
  plane root too: `charter` asks it to name a workspace with `-w` instead of acting on the
  plane's default, which now answers only for a caller outside the plane, and
  `purlis workspace use` still moves such a session. A root chat's footer
  (`charter statusline`) shows `⬢ plane root` rather than a workspace; a handoff from it is
  stamped `plane root` (the new chat still starts in the workspace you handed it to); and a
  report back to a root chat that has closed is kept for the next chat started at the plane
  root.
- **Curation actions: chats that open with their prompt typed, for you to read and send.** In
  the app, right-click a workspace, a persona or the plane root for **Curate ▸**, or type
  `curate` in the palette. The chat opens on your default profile as the action's persona,
  named `<action> · <subject>`, and the prompt appears in its input once the harness has
  started — never sent. If you type, paste or click in that chat before the prompt appears, it
  is dropped rather than typed after what you began. An action a persona's file could not
  offer is listed, greyed, with why. It needs a Claude Code or Codex default profile. Codex
  says nothing until your first prompt, so its prompt is typed once its terminal has gone raw
  and then stayed quiet for a second. opencode goes quiet while it is still starting, and a
  prompt pasted then is lost, so purlis has no moment to type into it and the menu says so
  (ADR 0061).
- **purlis ships three curation actions, and a persona can add its own.** Safe remove,
  Compact & improve, and Add curation action each name a new skill in purlis's plugin
  (`safe-remove`, `compact`, `add-curation-action`), so the prompt you read is one line — for
  example *Use purlis's safe-remove skill to remove the workspace alpha.* — and the skill
  holds the steps. A persona adds its own as `personas/<name>/curation/<id>.md`, or with
  `purlis persona curation add`. `purlis curation show workspace:<name>` (or
  `persona:<name>`, or `plane`) lists what a subject is offered, who runs each action, where,
  and the exact prompt. A prompt types a literal brace as `{{` or `}}`, so `{{word}}` types
  `{word}`. Claude Code shows a paste over 800 characters or of 4 lines or more as
  `[Pasted text #N +M lines]`, and Codex one over 1,000 characters as
  `[Pasted Content N chars]`, so an action whose prompt would be shown that way opens no chat
  on that harness and says why, and `purlis persona lint` warns about it, naming the harness.
  `purlis persona lint` also reports an action that is broken or that takes a built-in's
  name. `charter` is now a reserved persona name, because its actions would read
  `charter/<id>` like the built-ins: `purlis persona create purlis` is refused, and a persona
  that already has the name is reported by `purlis persona lint` and offers no curation
  actions (ADR 0061).
- **Plain shell tabs, and a warning when a harness starts inside one.** `New shell` sits beside
  `New tab` in the palette, on the panes' menu and on each workspace's menu (`New shell in
  <workspace>`), on ⌘⇧T (Ctrl+Shift+T off a Mac): your own shell, where a new chat would start,
  with a terminal's mark on its tab and no chat-state mark. Typing `claude`, `codex` or
  `opencode` in one still starts it, after one line saying it runs outside purlis's session
  tracking, and the tab shows a banner whose **Open as chat** opens the picker there with that
  harness picked. Detection is the command being started — purlis's shims stand first on a
  shell tab's `PATH`, and stay first after zsh's and bash's own start files — and nothing
  reads what the harness prints (ADR 0062).
- **Drag a tab to reorder it, in every strip.** Projects in the title bar, workspaces and chats
  each take a drag with the pointer, or from the keyboard: Shift+Space picks the focused tab
  up, the arrows move it, Space or Enter drops it and Escape puts it back. Dropping a tab
  across the pinned boundary pins or unpins it, where you dropped it, and a pin the store
  refuses goes back with the reason. A click is still a click: a drag starts only once the
  pointer has moved. Each order is kept on this machine and comes back on relaunch (ADR 0039,
  ADR 0040).
- **Find in a chat's pane, and Shift+Enter for a new line.** ⌘F (Ctrl+Shift+F off a Mac, so
  Ctrl+F stays with the shell) opens a find bar over the focused pane: every match is
  highlighted, Enter and Shift+Enter step through them with a count, and Esc closes it and
  puts the keyboard back in the terminal. In a Claude Code, Codex or opencode chat, Shift+Enter
  now adds a new line to the prompt instead of sending it; in a shell it is Enter, as before.
- **Create and delete vaults, personas and todos in the window.** A `+` on the Vaults and
  Personas headings opens *New vault…* and *New persona* (name, role, the work that comes to it,
  and what it inherits from; it is created as a draft and its tab opens). *Delete vault…* lists
  the secrets the vault holds by name, says plainly that a keychain vault's secrets are
  destroyed and cannot be recovered (a plain-file, references or 1Password vault's file or item
  is left where it is), and stays disabled until you type the vault's name. *Delete persona…*
  runs `purlis persona remove`, never forced, so a persona another one extends or uses is
  refused with their names. *Edit persona.md* opens the file in the app your system opens `.md`
  files with. The Todos panel has a box for a new todo in the focused workspace, and each todo's
  menu has *Mark done* and *Forget*; the window refuses a todo with the same rules and words as
  `purlis ws todo`.
- **Codex and opencode chats get purlis's skills too.** `safe-remove`, `compact`, `handoff`
  and the other six reached only Claude Code chats. An opencode chat the app starts now finds
  them as its own skills, beside any `skills.paths` your `opencode.json` names, and a Codex chat
  is told about each one when it starts, with the file to read, since Codex cannot take a
  skills folder for one session. Nothing is written into `~/.codex` or `~/.config/opencode`
  (ADR 0063).
- **The harness asks you before `purlis report` files an issue.** `purlis init` now writes an
  ask rule for `purlis report *--yes*` in `.claude/settings.json` and `opencode.json`, beside
  the one for `purlis handoff`, so a chat cannot file a public report without your yes.
  `purlis reinit` adds it to an existing plane and carries it into your workspaces.
  `purlis doctor`'s `ask rules` row warns when it is missing, and `purlis guard report` or
  `purlis doctor --fix` puts it back. So `--fix` now writes the plane's committed harness
  settings too, not only this machine's. Codex has no rule that can say this, so nothing is
  written there (ADR 0059).
- **`purlis change` declares a piece of work that spans several repos.** `create` names it
  and says why, `add` puts in a repo already cloned in the workspace (on `change/<slug>` or a
  branch you name, with `--needs` for the repos that must land first), `drop` takes one out
  with the reason, and `list`, `show` and `forget` read and end it. The record is
  `workspaces/<ws>/changes/<slug>.json` and holds intent only; it is committed when the
  workspace is LIVE. An unknown change, a repo with no clone, a repo added twice or an order
  that cannot be true is refused with exit 2. Pushing, landing and reverting come later
  (ADR 0060).
- **`purlis doctor` checks cross-repo changes in every workspace.** Its `changes` row fails on
  a change record purlis cannot read, naming the file and what is wrong with it, and on a
  change's branch sitting in a clone that is a member of no change. It reads only this disk and
  says which `changes/` directory it could not look at.
- **`purlis change show` says where each member's pull request stands.** Under the record it
  prints each member's request number, whether it is open, merged or rejected, and its checks at
  the request's exact head commit: PASSED, FAILED, RUNNING, NOT RUN or UNKNOWN. Zero checks is
  NOT RUN and a check purlis could not read is UNKNOWN, and neither is ever shown as passing.
  It says which members still wait on a blocker, and when the reading was taken. If the forge
  cannot be asked, the record still prints and each member says why. Nothing it reads is
  written back.
- **A workspace's cross-repo changes open in a tab.** "Open changes" in the palette (`F2`)
  opens a tab for the focused workspace. It shows each change, each member's branch, its pull
  request and its checks at the head commit, which members are blocked, and when that was read.
  It asks the forge when the tab opens and when you press Refresh, and never when you switch
  workspaces. A workspace with no changes says how to create one.
- **purlis's plugin teaches personas, vaults and the browser again.** It now ships the
  `charter:persona`, `charter:secrets` and `charter:browser` skills beside `handoff`,
  `update` and `working-in-a-clone`, rewritten for this purlis's commands. `purlis browser
  install [--version X.Y.Z]` generates Playwright's own page-driving skill into the plane's
  `.claude/skills/playwright-cli/` with `npx`, and gitignores `.playwright-cli/`, where traces
  of logged-in runs land. The version must be an exact version: anything else npm would read
  there, such as a tag or a git URL, is refused.
  ([#370](https://github.com/diazoxide/charter/issues/370))
- **Worktrees can be cut, declared and removed from the command line.** `purlis worktree`
  (alias `wt`): `add <repo> <piece>` cuts a piece off the clone's HEAD and records the claim;
  `done`, and `abandon "<why>"`, run from inside a piece, say it is finished or given up;
  `list` shows each piece with what it declared or how long it has been silent; `history`
  shows what happened to pieces, removed ones included; `remove` takes a piece away through
  git. The session briefing and the footer now see those declarations. In the window, each
  worktree row shows the same word, and its menu can mark it done.
  ([#368](https://github.com/diazoxide/charter/issues/368))
- **A persona's memory can be kept up like a workspace's.** `purlis persona forget <name>
  <slug>` deletes one memory, `purlis persona dedupe` lists near-duplicate pairs to prune,
  `purlis persona optimize` runs the curation `purlis workspace optimize` runs over each
  persona's memory and the shared store (read-only unless `--apply`), and `purlis persona
  log` notes to, or shows, a persona's activity in this session.
  ([#366](https://github.com/diazoxide/charter/issues/366))
- **A persona can be made, read, cleared and removed from the command line.** `purlis persona
  create <name> --delegate-when "<the work that comes to it>"` writes
  `personas/<name>/persona.md` as a draft, with its memory and refs; `--extends` inherits from
  another persona, `--with-vault` registers its vault and `--use` selects it. `purlis persona
  show` prints what a persona adopts, `purlis persona clear` drops your selection, and
  `purlis persona remove` refuses while another persona still extends or uses it (`--force`
  overrides). `purlis persona lint` finds dangling `uses:`/`extends:`, keys purlis cannot
  read, a missing role, vault or `delegate-when`, and stale generated sub-agents, and `purlis
  doctor`'s `personas` and `persona grant` rows now run it instead of saying "not checked".
  A refusal about a missing persona suggests `purlis persona create` again.
  ([#365](https://github.com/diazoxide/charter/issues/365))
- **The app starts opencode chats.** Pick an opencode profile in the new-chat picker and the
  chat runs with purlis's guard: a tool call the guard refuses does not run, and opencode is
  told why. The chat shows when it is working, when opencode asks your permission, and when its
  turn ends. It also gets the briefing and handed-back reports with your prompt. Nothing is
  written into opencode's configuration for this. An opencode chat reports nothing before your
  first prompt, and it reads waiting after you answer a permission prompt until its turn ends.
  A profile that passes `--pure` would load no plugin, so it is refused with the reason.
  `purlis plugin install --harness opencode` installs the guard for opencode chats you start
  in a terminal, and replaces the retired Python charter's opencode plugin if it is there.
  ([#371](https://github.com/diazoxide/charter/issues/371))
- **A workspace can be renamed.** `purlis workspace rename <old> <new>` (or `mv`), or *Rename
  workspace…* on the workspace tab's menu and in the palette. The folder moves, every git
  worktree of its clones is repaired so it keeps working, and everything that names the
  workspace follows: its manifest, the LIVE list, the default and each session's choice, the
  app's open tabs and what a relaunch reopens, and your pins. A LIVE workspace is saved once
  afterwards. It is refused while a chat is running in the workspace, naming the chats, and when
  the new name is taken or is not a valid name. Unpushed or uncommitted work is not a reason to
  refuse, because a rename moves it whole. If a rename is interrupted, running the same command
  again finishes it. Claude Code keeps a conversation under the folder it ran in, and purlis
  does not move that folder, so `purlis workspace rename`, and the Rename dialog before you
  confirm, list by name each Claude Code chat in the workspace that will start a fresh
  conversation. When you reopen one of those chats, it starts a new conversation and says why
  once, instead of failing to resume. Codex and opencode chats resume as before and are not
  listed. ([#367](https://github.com/diazoxide/charter/issues/367))
- **`purlis report bug` and `purlis report feature` file an issue on purlis's own tracker.**
  Each run shows the draft and files nothing. To file it, answer `y` at the prompt in a
  terminal, or run the same command again with `--yes` and the digest the draft printed. If the
  draft has changed since, nothing is sent. The issue is filed under your own `gh` login, never
  under a token from the environment. Before you see the draft, purlis removes secrets,
  environment values, your plane's path, home paths and the names of your workspaces, repos,
  personas and vaults, and says what it removed. It also lists possible duplicates.
  `purlis report bug --panic` drafts the last panic the app saved, with where it happened and
  the purlis version, which panic records now include.
  ([#363](https://github.com/diazoxide/charter/issues/363))
- **A project tab can move into a window of its own, and back.** Right-click a project tab, or
  use the palette, and choose *Move project … to a new window*. Its chats keep running. In that
  window, *Move project … to the main window* brings it back, and so does closing the window.
  Each window has its own palette and project tabs. The ✋ list still shows every chat that needs
  you, whichever window it is in, and pressing one takes you to that window. Quitting warns
  about the chats in every window. The next launch opens each window again.
  ([#126](https://github.com/diazoxide/charter/issues/126))
- **`purlis guard` is back: rules that always ask, or stop asking.** `purlis guard ask
  '<pattern>'` and `purlis guard allow '<pattern>'` write the rule in each harness's own
  file: Claude Code's `.claude/settings.json`, or `.claude/settings.local.json` with
  `--local`, and opencode's `opencode.json`. They touch nothing else in either file, and if
  one of those files cannot be read, they write nothing anywhere. `purlis guard handoff` puts
  back the handoff consent rule a plane lost, and `purlis guard` on its own lists the rules
  by file. `purlis doctor` now checks its `handoff gate` and `ask rules` rows instead of
  saying "not checked". ([#364](https://github.com/diazoxide/charter/issues/364))
- **`purlis save --pull` brings in what the remote has before it saves.** A chat the app did
  not start, such as a `claude` or `codex` in a terminal, gets no auto-save and no incoming
  changes. Outside the app, the plane is saved only through `purlis save`. `--pull` fetches
  the plane's target branch and fast-forwards a clean tree first, the same way the app does.
  With unsaved work in the tree, what came in is left alone and the save still runs. If the
  tree has conflicts, or there is no remote to pull from, the command stops and saves nothing.
  ([#375](https://github.com/diazoxide/charter/issues/375))
- **A chat's tab says when the plane's instructions changed after it started.** A running chat
  keeps the `CLAUDE.md`, harness settings, sub-agents and persona charter it read when it
  started. When one of them changes on disk, its tab gets a quiet mark naming the files, so you
  know it will run on the old ones until you start it fresh. It is not a needs-you item.
  ([#369](https://github.com/diazoxide/charter/issues/369))
- **The commitment gate is back.** When a prompt asks for work and leaves a real choice open
  (open-ended wording, a broad scope, something irreversible, a long many-part ask), the chat
  is told to look first and then ask you at that choice before it builds. It stays quiet for
  questions, for work with nothing to ask about, for slash commands and for unattended runs,
  and for the three prompts after it fires.
  ([#369](https://github.com/diazoxide/charter/issues/369))
- **`purlis plugin install` guards the `claude` and `codex` chats you start in a terminal.**
  The app arms only the chats it starts, so a terminal chat ran purlis's guard only if the
  retired Python charter's plugin happened to still be installed. `purlis plugin install`
  registers the app's own plugin with Claude Code, and purlis's Bash guard with Codex, for
  every chat on this machine. It prints each change, `--dry-run` shows them without writing,
  and a second run changes nothing. It never turns on the retired `charter@charter` plugin,
  and turns it off where it writes. `purlis plugin uninstall` takes it back.
  ([#374](https://github.com/diazoxide/charter/issues/374))

### Changed

- **The window's chrome no longer selects as text.** Tab names, headings, buttons, menus and
  rows stay put when you click or drag across them, as a native app's do. What you would copy
  still selects: fields, code, paths, names and commands, a view's answer, error text, and a
  dialog's text. Pulling on a link or an image no longer drags out a ghost of it. A terminal
  selects as it always did.
- **Removing a worktree that holds work now says which work.** The refusal lists the
  uncommitted files and the commits no other branch has, so you can see what `--force` (or
  "Discard that work and remove the worktree anyway", in the window) would discard. A merge
  refused over uncommitted changes no longer tells you to remove or force.
  ([#368](https://github.com/diazoxide/charter/issues/368))
- **The app keeps the plugin you installed for terminal chats up to date.** When it starts,
  it re-runs `purlis plugin install` for each harness (Claude Code, Codex, opencode) whose
  installed copy runs the app's own `charter` and is older than what the app ships. It never
  installs for a harness you did not install for, and leaves a copy that runs another
  `charter` alone. ([#449](https://github.com/diazoxide/charter/issues/449))
- **`purlis guard ask` puts a new rule into your workspaces straight away.** It rewrites the
  generated settings of every workspace, as a launch or `purlis workspace reinit` would,
  and names any workspace whose settings it could not rewrite. Before, the rule reached a
  workspace only after `purlis workspace reinit --all`.
  ([#449](https://github.com/diazoxide/charter/issues/449))
- **A vault file that is a symlink is refused.** `purlis secret set` and `purlis secret rm`
  on a plain-file or reference vault whose file is a link now stop with a message saying so,
  and write nothing. They used to write the secrets to wherever the link pointed. Point the
  vault's `file` at the real path instead.
  ([#429](https://github.com/diazoxide/charter/issues/429))
- **Releases are signed from a protected `release` environment.** The release workflow's
  signing and publishing jobs now run in a GitHub environment that only `main` and `v*` tags
  can reach, and the signing keys live there instead of in the repository. A build started by
  hand from the Actions tab publishes nothing and is now always unsigned for the updater.
  `docs/updating.md` has the setup.
- **`purlis news` prints this changelog.** It shows every version of the app, newest first,
  and `purlis news --for <version>` shows one, the same notes as the release page and About
  purlis. It used to read the Python charter's news and told every plane it had no update
  baseline. `--since`, `--until` and `--pending` are retired and say what to run instead.
  `purlis update` points at `purlis news`, and the pin dialog no longer has an empty news
  list.
- **Pinned workspaces stay in the order you pinned them.** The workspace strip draws them in
  that order, and a workspace you pin later goes after the others, until you drag it. Unpinning
  one leaves the rest where they were. Pins from an earlier version keep the order they had.
  ([#402](https://github.com/diazoxide/charter/issues/402))
- **The project strip's show-more menu lists the most recently active projects first**, after
  the ones that need you. It used to list them in the strip's order.
  ([#401](https://github.com/diazoxide/charter/issues/401))
- **The title bar shows incoming commits as their own `↓N`.** It sits after the save
  indicator's words and is never cut off when the words are. A long stage is still cut short
  in the bar and read in full from its tooltip or the Saving view. The window can no longer be
  made narrower than 1024 px; at that width the title bar keeps room for two project tabs.
  ([#403](https://github.com/diazoxide/charter/issues/403))
- **`routing:` in a persona is retired.** Personas are offered to the harness as sub-agents,
  which is where work is routed. A persona that still declares `routing:` loads as before,
  `purlis doctor` says the key is ignored, and `purlis init` no longer writes it.
  ([#369](https://github.com/diazoxide/charter/issues/369))

### Fixed

- **A chat started in a workspace is no longer asked which workspace it is in.** The app filed
  it under its workspace and told it nothing, so its briefing asked you to confirm one. Every
  chat the app starts now carries `$CHARTER_WORKSPACE` in a workspace, or
  `$CHARTER_PLANE_ROOT_SESSION=1` at the plane root, and never inherits either from whatever
  launched the app. `charter` also counts a workspace's own directory, `workspaces/<ws>`, as
  being in that workspace, as the window already did.
- **A chat's terminal scrolls as far as your fingers move, from the first pixel.** A trackpad
  or wheel now moves the history one row for every row's height of travel, and a harness in
  full screen — Claude Code's `"tui": "fullscreen"`, opencode — is sent one wheel report per
  row, what is left over carried to the next event. Before, a slow start barely moved a
  full-screen harness (one report per ~50 px) and a flick was cut to one report per event, so
  scrolling felt slow to start and then fast.
- **A program that asks for mouse reports in the default encoding gets them.** A full-screen
  program in a chat's terminal that turns on mouse tracking without SGR reports got no clicks
  and no wheel from the pane; they were dropped. They now reach it, byte for byte, columns and
  rows past 95 included. Claude Code and opencode use SGR reports and were never affected.
  ([#493](https://github.com/diazoxide/charter/issues/493))
- **The prose guards treat a process substitution as the substitution it is.** A `gh` or
  `glab` command that publishes prose, a purlis command that persists it, and `purlis
  handoff` now refuse `<(…)` and `>(…)` wherever the shell runs them, and zsh's `=(…)`, exactly
  as they refuse `$(…)`. Quoted, or in a heredoc body, they are text and are left alone. The
  refusal names a process substitution rather than calling it a command substitution.
- **Where the shells may disagree about a heredoc, the guards read it both ways.** Inside
  `(( … ))`, `$(( … ))`, `$[ … ]`, `${ … }` or an array subscript, `<<` can be a shift rather
  than a heredoc; and when a heredoc is opened inside `$(…)`, backticks, `<(…)` or `>(…)` and
  the substitution does not close on that line, bash 3.2, bash 5 and zsh can disagree about
  which of the following lines are its body. In both cases the guards now read those lines both
  as commands and as a heredoc body, and never set them aside as a body alone. After such a
  body, or after a heredoc whose delimiter the shells read differently, they no longer set
  aside any later heredoc body either. The leak guard also reads the command inside zsh's
  `=(…)` as a command of its own, as it already did for `<(…)`, and a heredoc opened inside a
  process substitution that closes on the same line is read the way a heredoc inside `$(…)`
  already was.
- **The nightly mutation run finishes again.** Its shards were sized for a test suite half as
  long as today's, so two of them ran out of time. The run now uses smaller shards and a longer
  per-mutant limit, and its slowest test, the plane-root replay, takes about a quarter of the
  time it did because it asks git each distinct question once instead of once per recorded row.
  It also stops reporting slow survivors as timeouts. New tests now cover the extension,
  executor, secrets, save, settings and `CHARTER_*` steering behaviour the run found untested.
  ([#464](https://github.com/diazoxide/charter/issues/464))
- **`purlis plugin uninstall --harness codex` no longer leaves Codex's trust record for the
  guard behind** in `[hooks.state]`. A record for a hook of yours that sat after the guard is
  moved to its new position, so Codex does not ask you to trust it again.
  ([#449](https://github.com/diazoxide/charter/issues/449))
- **`purlis doctor` quotes every path and your git identity on one line.** A newline, a
  carriage return or a terminal escape in the plane's path, the working directory,
  `$CLAUDE_CONFIG_DIR` or your git `user.name` and `user.email` is shown escaped instead of
  reaching your terminal. ([#449](https://github.com/diazoxide/charter/issues/449))
- **Every guard reads a heredoc the same way, and the way the shell does.** The secret-leak
  guard used a second, narrower reading of where a heredoc starts than the one that decides
  where its body ends, and on some lines the two disagreed, so a command after the heredoc could
  be taken for part of its body. There is now one reading. It also understands delimiters it
  used to miss: one with a blank in it (`<<'A B'`), one in ANSI-C quoting (`<<$'…'`), and one in
  double quotes that holds an escape or a backslash-newline. In a heredoc that expands, a line
  joined to the one before by a trailing backslash no longer ends the body. The lines after a
  heredoc opened inside a `$( … )` or backticks that close on the same line are read as
  commands too, because bash 3.2 and zsh run them. The body of `purlis handoff` spelled in
  other letter cases (`CHARTER handoff`) is read as its brief, as it is for the plain spelling.
  ([#359](https://github.com/diazoxide/charter/issues/359))
- **A terminal pane that opens late no longer adds a line when a wide character sits in the
  last column.** If a program had pushed a wide character, such as a CJK character, into the
  last column with wrapping turned off, the catch-up redraw printed that character again. That
  wrapped it onto the next row, and on the bottom row it scrolled the pane by one line. The
  pane now draws the blank that the terminal holds there.
  ([#435](https://github.com/diazoxide/charter/issues/435))
- **A pane that opens late shows what a pane that was open all along shows, around wide
  characters.** When a program deleted, inserted, erased or wrote over half of a wide
  character, such as a CJK character, the open pane blanked it and a pane opened later still
  showed it, until the program redrew that row. The same could happen after a program deleted
  or inserted characters, or moved a row down or up, right after writing the last column: the
  next character landed in the last column in one pane and on the next row in the other. A few
  more cases, such as deleting more characters than the row had left, now come out the same in
  both panes too. ([#441](https://github.com/diazoxide/charter/issues/441))
- **A pane that opens late puts the cursor where a pane that was open all along puts it.** A
  tab, or a restored cursor, right after a program wrote the last column sent the next
  character to the next row in one pane and not in the other. So did inserting or deleting
  lines, which move the cursor to the first column, and moving the cursor up or down inside a
  scroll region, which stops at the region's edge. A restored cursor now also comes back to
  the same row in both panes after the screen scrolled. A pane that opens late now also keeps
  the program's scroll region, so a full-screen program such as an editor or a pager scrolls
  the right rows in it. ([#447](https://github.com/diazoxide/charter/issues/447))
- **A pane that opens late no longer shows a screen you cleared in its scrollback.** After
  `clear`, or after a program deleted lines at the top of the screen or scrolled it up, a pane
  that opened later had the old lines in its scrollback, and a pane open all along did not.
  Now both panes show the same scrollback.
  ([#452](https://github.com/diazoxide/charter/issues/452))
- **The extension and `secrets exec` tests no longer fail on a busy machine.** Extensions
  still get the same time as before: 5 seconds for a view, an action, an event or a command,
  and at a chat's start 2 seconds each and 3 seconds for all of them together; a program
  `secrets exec` runs still gets a quarter of a second to finish after a Ctrl-C. A debug build
  of `charter` now lets the test suite set a different limit, so a test that is not about the
  limit gives a slow machine room, and a test that is about it uses a short limit and a program
  that never answers. A test whose program has to get somewhere before the limit now waits for
  it to get there, and the one `secrets exec` test that replaces its own process runs apart
  from the others. ([#422](https://github.com/diazoxide/charter/issues/422),
  [#465](https://github.com/diazoxide/charter/issues/465))
- **The guards read more of the shell's quoting the way the shell does.** The shared command
  reader behind every guard now decodes ANSI-C quoting (`$'…'`) and bash's `$"…"` strings, and
  drops a backslash-newline line continuation before it reads a word, inside double quotes as
  well as outside. A command substitution split by a line continuation is recognised as one,
  and so is bash 5.3's `${ …; }`. A heredoc delimiter split by a continuation is read as the
  unquoted delimiter it is. A shell named in capitals (`BASH -c`) is recognised when it runs a
  string, as it is on a filesystem that ignores case. A git alias that runs through the shell
  is read with the same rules.
- **A save no longer commits unresolved conflicts.** When git has stopped part-way through a
  merge, rebase, cherry-pick, revert or bisect, or files still have conflicts, every save now
  refuses and stages nothing: `purlis save`, the Save button, auto-save and repo saves alike.
  It says which files conflict and the git command that finishes or aborts the operation. The
  Saving tab shows the plane or repo as Blocked until you do, and auto-save waits.
  ([#433](https://github.com/diazoxide/charter/issues/433))
- **The dispatch log, the session trace and a memory index refuse to write through a link.**
  They now open the file without following a link, and refuse it when it is one.
  ([#420](https://github.com/diazoxide/charter/issues/420))
- **A vault is never left half-written.** Setting or removing a secret in a plain-file or
  reference vault now writes a new file beside it and swaps it in, instead of rewriting the
  vault in place, so a crash or a full disk mid-write leaves the old vault whole. The file is
  still private to you (0600) from the moment it exists.
  ([#429](https://github.com/diazoxide/charter/issues/429))
- **Every file purlis replaces whole is written the same careful way.** `workspace.json`, the
  settings purlis generates for a workspace or a checkout, the profile approval record and
  the hook bookkeeping now share one writer. Each is flushed to disk with its directory, keeps
  the permissions it had (or stays private, for purlis's own state), and is never replaced
  when it is a symlink. A `workspace.json` or generated settings file you made read-only is
  now left alone and reported, where it used to be replaced.
  ([#430](https://github.com/diazoxide/charter/issues/430))
- **More of purlis's files are replaced whole and never written through a symlink.** The
  vault registry (both halves), the fingerprint key, memory files and a memory index's
  rewrite, the front-door persona charter scaffolds, a checkout's presence record, the
  remembered open chats (`reopen.json`), the app's machine store, extension record and window
  layout, and purlis's own state files (the active persona and workspace, MCP approvals, the
  forge cache and its lock) now go through the same writer: a new file beside the old one,
  flushed and swapped in. A crash mid-write leaves the old file whole, and a file that is a
  symlink is refused, where some of these used to write to wherever the link pointed and
  others replaced the link. What else changes:
  - The local vault registry and the fingerprint key must be private (0600). On a filesystem
    that cannot hold that mode, the write is now refused instead of made at a looser mode.
  - The shared vault registry keeps the permissions it has, where it used to be reset to 0644
    on every write. A new one gets your usual file permissions.
  - `reopen.json` is now private to you (0600). It used to get your usual file permissions.
    So is a memory index under `.charter/` when purlis removes a line from it.
  - purlis's own state files that you made read-only are replaced, as purlis owns their
    mode. A read-only shared vault registry, local registry or fingerprint key is refused.
  - A `.gitkeep` that is a symlink stops `purlis init`'s front-door persona with an error.
  ([#434](https://github.com/diazoxide/charter/issues/434))
- **`purlis doctor` knows the Python charter is retired.** Its `python3` row no longer warns.
  Its three plugin rows, which said "not checked", now check purlis's plugin for chats started
  outside the app: whether it is installed, whether it is current, and whether the `charter` its
  hooks run still exists. A new `superseded plugin` row names every settings file that still
  turns on the retired `charter@charter`. `purlis doctor --fix` works again: it runs
  `purlis plugin install`, prints each change, then reports. A workspace or worktree layer no
  longer copies `charter@charter` from the plane's settings.
  ([#373](https://github.com/diazoxide/charter/issues/373))
- **purlis's private files are read without following a symlink, too.** The fingerprint key,
  both halves of the vault registry, a plain-file vault and its rotation record, a keyring
  vault's key index, and the hook bookkeeping (the session's tool ceiling, the commit-gate
  and memory counters, the sub-agent map and the running-dispatch records) are now refused
  when they are a symlink or reached through one, where they used to be read from wherever
  the link pointed. What each does then:
  - A fingerprint key that is a symlink gives no fingerprint; `secret get` shows only the
    size band.
  - A vault registry, vault file or key index that is a symlink is an error that names it.
  - A session tool ceiling that is a symlink grants nothing, so every tool asks.
  - When a vault registry half is a symlink, the persona tool gate does not auto-allow a
    command, since it cannot tell which files are vaults.
  - Other bookkeeping reads as nothing recorded.
- **A `.charter/` directory that is a symlink is refused for the vault registry and the
  fingerprint key.** Neither is read from nor written to where it points. A `$CHARTER_HOME`
  outside the plane is still used as you set it.
- **`purlis reinit` reports a `workspaces/.gitkeep` that is a symlink.** It used to take it
  as present when the link stayed inside the plane or pointed at nothing. Now it is an error,
  as a persona's `.gitkeep` already was.
- **Temp files an older purlis left behind are cleaned up.** An older purlis killed
  mid-write could leave `reopen.json.writing` or a temp of the profile approval record in a
  plane's `.charter/`, or a `*.writing` temp beside the app's machine store, extension record
  or window layout. Nothing removed them. The app now deletes them when it opens a plane:
  only those exact names, only plain files, and only when they are more than ten minutes old.
  ([#440](https://github.com/diazoxide/charter/issues/440))
- **A guard that crashes now refuses the tool call instead of letting it run.** If purlis hit
  an internal error while checking a tool call, the crash ended the process with a status
  Claude Code and Codex read as a non-blocking error, so the call went ahead unchecked. Any
  crash in a `PreToolUse` hook now exits 2, which both read as "block", with one line on
  stderr saying the guard could not answer. A crash in any other hook still never blocks.
  ([#349](https://github.com/diazoxide/charter/issues/349))
- **A handoff leaves a todo in the workspace it went to.** Once the app has opened the new
  chat, `purlis handoff` records a todo there: the brief's first line and which chat handed it
  off, never the rest of the brief. If a todo about the same work is already open there, it
  says so and records nothing twice. It also adds one row to the dispatch log saying whether
  the chat went to this workspace or another, and whether the handoff created it; the row
  names no workspace, persona or brief. A write that fails is said and never undoes the open.
  ([#372](https://github.com/diazoxide/charter/issues/372))
- **`purlis doctor` quotes a value it read from a file, a folder name or the environment on
  one line.** The plane root row's memory-push record, the session layer row's harness name,
  and the workspace and persona names in the clone and memory rows used to be printed as they
  were, so a value with a line break in it could print a row that looked like one of doctor's
  own. ([#353](https://github.com/diazoxide/charter/issues/353))
- **`charter.toml` and the plane's `.gitignore` can no longer be left cut short.** purlis
  now writes the new version beside the file and swaps it in, so a crash or a full disk leaves
  the old file whole. Two edits at once, such as `purlis persona default` while the settings
  tab saves, or two workspaces made live together, now both land instead of one overwriting
  the other. A `.gitignore` purlis cannot read as text is now left alone rather than
  rewritten from nothing. ([#357](https://github.com/diazoxide/charter/issues/357),
  [#358](https://github.com/diazoxide/charter/issues/358))
- **`purlis init` creates `workspaces/.gitkeep`**, the file its `.gitignore` already
  expected, so an empty `workspaces/` can be committed. `purlis reinit` adds it to older
  planes. ([#355](https://github.com/diazoxide/charter/issues/355))
- **A reference vault's file is private from the moment it is created.** purlis used to
  write it first and restrict its permissions afterwards; it now sets them before any content,
  as plain-file vaults already did.
  ([#356](https://github.com/diazoxide/charter/issues/356))
- **`purlis doctor` describes Codex correctly.** Its session layer row said Codex ignores a
  project `.codex/config.toml` and gets purlis's layer from a plugin. The app arms Codex with
  flags on each chat's command line, and Codex reads a project's `.codex/config.toml` once you
  trust the project. The harness guide says the same.
  ([#354](https://github.com/diazoxide/charter/issues/354))
- **The plane root's branch guards can no longer be walked past by spelling.** A branch switch
  or a commit-destroying `git reset` in the plane root was let through when `git` was typed in
  capitals (`GIT`, which runs git on macOS and Windows) or with quotes inside it (`g''it`), when
  the root was named with different letter case or through `/System/Volumes/Data`, when the
  command ran from a folder inside the root such as `docs/`, when `env -C` or `sudo --chdir`
  moved it there, or when an alias was defined in one case and used in another. All of these
  are refused now. ([#346](https://github.com/diazoxide/charter/issues/346))
- **A `cd` that fails no longer hides the command after it.** `cd somewhere; git checkout x`
  was judged as running in `somewhere` even when the `cd` failed and git ran in the plane root.
  Now only `cd somewhere && …` counts as having moved, and only up to the end of that `&&`
  chain. A `cd` in a pipeline or a subshell, `pushd`, `~`, and a destination purlis can't read
  (`cd "$DIR"`, `cd -`) are followed the way the shell follows them.
  ([#345](https://github.com/diazoxide/charter/issues/345))
- **`GH issue create` and `CHARTER persona remember` are checked like their lower-case
  spellings.** The check that refuses a live `` `…` `` or `$(…)` in a forge body or a purlis
  memory skipped a program name typed in capitals or split by quotes.
  ([#347](https://github.com/diazoxide/charter/issues/347))
- **An unattended run can no longer publish just because it listed the tags first.** Listing
  or deleting local tags in the same command as a release, a tag, a tag push or a merge no
  longer lets that command past the release floor.
  ([#348](https://github.com/diazoxide/charter/issues/348))
- **The secret-leak guard reads search options the way the search tools do.** A `--glob` that
  selects files is no longer taken for one that excludes them. A program or pattern read from a
  file with `-f`/`--file` is checked like any other file the command opens. A search's pattern
  and file options are read however they are spelled: with `=`, bundled together, shortened, or
  after `--`. `rg`'s, `grep`'s and `ag`'s other options that take a value are read too.
  ([#350](https://github.com/diazoxide/charter/issues/350),
  [#351](https://github.com/diazoxide/charter/issues/351))

## [0.3.0] - 2026-09-25

0.3.0 is about extensions you can act through and workspaces that carry their repos. An
extension can now add commands to `charter`, palette entries and row actions, hear what happens
and add to a chat's briefing, and show badges and repo columns, each capability named in the
approval dialog; persona statistics ships built in. A workspace picks its repos when you make it
and saves each one by its own mode, and a project says what is not saved yet and carries its
commits on. It is also the first release from the repository's new name, `diazoxide/charter`,
and purlis's own plugin is now called `charter`.

### Added

- **Pick a workspace's repos when you make it, and change them later.** The new-workspace
  dialog lists the repos your own `gh` or `glab` login can reach under the plane's forges,
  private ones included, and clones the ones you tick into the workspace after it is made. Each
  repo clones on its own, so you can start a chat while they land; one that fails says why and
  can be tried again. A workspace's settings have a Repos section with the same list: tick to
  clone, untick to remove. A repo with uncommitted or unpushed work, or a worktree, is never
  removed from there. If you're not logged in to a forge, the dialog says so and you can still
  make the workspace. (ADR 0055)
- **Extensions can add commands to `charter`.** An extension that asks for the `cli`
  capability runs as `charter <its id> <command> …`, from a terminal, a script or a chat. What
  its program prints and its exit status come back unchanged. Each command says whether it
  writes; the approval dialog lists the ones that do, and they are held to the plane paths the
  extension declares, with anything else they change reported. A chat's call goes through the
  same guard as any `charter` call, and a persona's grant never lets one that writes run
  without asking. An extension turned off, not yet approved or changed since you approved it
  says so and runs nothing. An extension can never take one of purlis's own words as its id.
  `charter <id>` alone lists its commands. The command line doesn't reach the app's built-in
  extensions yet. ([#342](https://github.com/diazoxide/charter/issues/342))
- **Extensions can hear what happens, and add to a chat's briefing.** An extension that asks
  for the `events` capability is told when a workspace is focused, created, forked or removed,
  when a handoff is made, when a chat starts and when the plane is saved. It is told after the
  thing is done, so a slow or broken extension never holds it up or changes how it went. It
  shows as a note naming the extension instead. A fork copies the folder an extension keeps in
  each workspace, even while the extension is off. One that asks for `briefing` adds a section
  to every chat's first message. The section is quoted under its name as data, not
  instructions, is cut at 1,500 characters, and is left out if it holds text that can't be
  drawn. The approval dialog says it "adds text to every chat's first message". A chat's start
  waits at most three seconds for all extensions together. Both need protocol 2.
  ([#343](https://github.com/diazoxide/charter/issues/343))
- **An extension can be acted on, not only read.** Three capabilities, each named in the
  approval dialog: `palette` adds commands to the palette, named with the extension's name, that
  open one of its views or run one of its actions; `actions` puts the extension's own actions on
  the rows of its views, and the answer can refresh the view; `writes` declares the plane paths
  it writes, such as `workspaces/*/todos/`. purlis asks before an action when the extension
  says to, and always before one that deletes. Each request tells the extension where it may
  write, and after each one purlis says what changed outside those paths, naming the
  extension. That is a report, not a fence: an extension still runs as you. The protocol is now
  2; an extension written for protocol 1 is asked exactly as before and keeps its approval.
  ([#341](https://github.com/diazoxide/charter/issues/341))
- **Workspace repos are saved too, each by its own mode.** The Saving tab has a row for every
  repo in the workspace: its stage, the branch it is on, its pull request and its own Save
  button, with *Save all* for the project and every repo at once. The title bar counts the
  workspace's repos in: *1 repo changed*. A repo is saved by `[repos.<name>] mode`, `pr` by
  default. A save commits on the branch the repo is on. `push` pushes that branch. `pr` pushes it
  and opens or updates a pull request into `branch`, the repo's default branch by default.
  `pr-merge` also asks for auto-merge, and says so when the forge won't queue it. On the default
  branch itself, a PR mode pushes a new `charter/<workspace>/<short-sha>` branch instead, so it
  never pushes to the default branch. A repo is saved only between turns. A save you press while
  a chat in that workspace is working is refused, with a sentence naming the chat. Auto-save
  skips that round. Repos are saved by themselves only when `[repos.<name>] autosave = true`,
  which is off by default, and quitting saves only those, and not one whose chat's turn the
  quit cut off. A pull request you opened yourself from the branch is never rewritten or set to
  merge. A repo save refuses a secret-shaped file (`.env`, a private key, `credentials.json`, a
  `.npmrc` with a token, …) or a private key or forge token in what it would commit, and names
  the file.
  ([#299](https://github.com/diazoxide/charter/issues/299))
- **Persona statistics comes with the app.** purlis now ships its own extensions, and persona
  statistics is the first: there is no folder to assemble and add by hand, and no approval to
  give, because the app's signature covers it. The Extensions list marks it "built-in" and
  offers turning it off on this machine in the place of Remove. A project or a workspace can
  still turn it off, as it can any extension. A copy of it anywhere else is an ordinary
  extension that has to be approved. If you added it by hand before, that copy is set aside and
  the built-in one is used. Its numbers are now `purlis persona stats`'s: it counts the same
  memories and dates them the same way, and "recent" means the last 14 days in both. For
  extension authors: a view about personas is now handed the day each memory was written,
  rather than its minute. ([#339](https://github.com/diazoxide/charter/issues/339))
- **Extensions can show badges and repo columns.** An extension that asks for the `badges`
  capability can show values in the status bar and in `charter statusline`'s footer, and one
  that asks for `repo-columns` can add columns to the repo table in the bottom bar. It declares
  each one in its manifest, with how long a value stays fresh, and the approval dialog lists
  them. The values come from a facts file the extension keeps in its state directory, and
  purlis never starts the extension's program to draw them. A value older than its freshness
  is dimmed and shows its age. A facts file that is too big, isn't JSON, or fills something the
  manifest didn't declare shows nothing and says why. So does an extension that changed since
  you approved it. Turning an extension off for a project or a workspace hides its badges and
  columns there. ([#340](https://github.com/diazoxide/charter/issues/340))
- **Every open project says whether it has unsaved work.** A dot on a project's tab marks work
  a save would take, or a save that is blocked (red), so a project behind the one in front is
  not where work is forgotten. Each project keeps its own save state and its own auto-save,
  and quitting saves every one of them.
  ([#302](https://github.com/diazoxide/charter/issues/302))
- **An extension says which capabilities it asks for.** An extension's `charter-extension.json`
  can list them in `capabilities`. The approval dialog and the Extensions list name each one,
  and changing the list asks you again. An extension that asks for a capability this purlis
  doesn't know is refused as a whole, with a sentence naming it. Each capability arrives in
  its own change. An extension with no `capabilities` loads exactly as
  before and keeps its approval. `version` in the manifest is now the protocol its program
  speaks. ([#338](https://github.com/diazoxide/charter/issues/338))
- **LIVE and LOCAL, from the window.** A LIVE workspace, whose purlis, memory and todos are
  published with the project, is marked on its tab, in the title bar and in the Explorer, and
  the Saving tab names the live ones. Its menu, the palette and its settings page offer
  *Make live…* or *Make local…*. Before anything changes, a confirmation says which files and
  where they go (the remote, or "this machine only"). The project is saved at once. Making a
  workspace LOCAL stops publishing its files and keeps them on disk; what was already pushed
  stays in history, and the confirmation says so. The new-workspace dialog has a *Live* box,
  unticked by default. ([#301](https://github.com/diazoxide/charter/issues/301))
- **A blocked save shows its way out.** When a save can't go further (a conflict with the
  remote, a secret the scan caught, a pull request mode on a remote purlis can't open pull
  requests on), the Saving tab says why, names the files a conflict is in, and offers
  *Resolve in a chat* (the chat picker, starting in the project) or *Open terminal here* (a plain
  shell in the project). The alerts drawer says so too: at once for a secret, and after ten
  minutes for anything else.
- **Fewer conflicts in the first place.** `purlis init` and `purlis reinit` write a
  `.gitattributes` block that merges the logs and memory indexes which only ever grow line by
  line, so two machines adding to the same one no longer conflict.
  ([#295](https://github.com/diazoxide/charter/issues/295))
- **Saving through a pull request.** A project whose `[plane] mode` is `pr` or `pr-merge` now
  saves the whole way. Each save commits on the project's branch, pushes it to this machine's
  own save branch (`[plane] save_branch`, `charter/save/<this machine>-<this clone>` unless
  you name one),
  and opens one pull request from there into `[plane] branch`. The next save updates that same
  pull request. `pr-merge` also asks GitHub or GitLab to merge it once its checks pass. If the
  forge will not queue the merge, the Saving tab says why and the pull request stays open for
  you. The Saving tab shows *Pushed — waiting on its pull request* with the link. Once the pull
  request has merged, by a merge commit, a rebase or a squash, purlis moves your branch onto
  the remote's and keeps anything newer you have not saved. If the pull request was closed
  without merging, or the branch no longer holds what was pushed, the project is **blocked**
  and nothing is moved; save again to open a new pull request. A file in the way of the move
  just waits for the next look. purlis force-pushes only its own save branch, and only over
  what that clone pushed there itself, so a second machine with the same name never
  overwrites the first's. It never pushes to the project's branch in these modes. ([#298](https://github.com/diazoxide/charter/issues/298))
- **Commits left behind are carried on.** In a project whose mode pushes, a save with nothing
  new to commit still pushes the commits this machine has not pushed yet. That covers a push
  that quitting did not have time for, and a commit a chat made with plain git.
- **Auto-save.** While purlis is open, a project with auto-save on (`[plane] autosave`,
  on by default) saves by itself: 30 seconds after the last change (`autosave_after`), as soon
  as a chat in it ends, and when you quit. At quit it commits at once and gives the push a few
  seconds; whatever did not get pushed is pushed the next time purlis opens the project. It
  pauses while a save is blocked, and a push that fails is retried every five minutes, not
  every 30 seconds.
- **What came in.** Every five minutes, and when the window comes back into focus, purlis
  fetches the project's branch. The title bar and the Saving tab say how many commits came in
  (*2 incoming*). With auto-save on, a project with nothing unsaved is fast-forwarded onto
  them. Otherwise they wait for your next save.
- **One question per project.** A project that has never said how it is saved (no
  `[plane] mode`, including every project whose `charter.toml` says `share = "local"`) is asked
  in the Saving tab: *Push*, *Commit only* or *Off*. The answer is written into `charter.toml`,
  and until there is one, nothing saves the project by itself.
  ([#296](https://github.com/diazoxide/charter/issues/296))
- **The title bar says what is not saved yet.** Beside the needs-you button, the project in
  front shows where its unsaved work sits: *3 changed*, *committed, not pushed*,
  *waiting on its pull request*, *blocked*, or *Saved*. A save button sits next to it while
  there is anything to save. Press the words to open the project's **Saving** tab, which lists
  the files the next save takes, lets you type a message (leave it empty and purlis writes one
  that says what changed), and shows the last 50 saves and how each one ended. The tab is also on
  the project tab's menu and in the palette, as *Saving…*. The button runs the same save as
  `purlis save`, so both follow `[plane] mode`.
  ([#294](https://github.com/diazoxide/charter/issues/294))
- **Project settings has Plane and Repos sections.** Both files, Shared (`charter.toml`) and
  Local (`charter.local.toml`), now have a **Plane** group — mode, target branch, save branch,
  signing, auto-save and how long auto-save waits — and a **Repos** group with the same keys
  (bar the save branch) for every repo in `inventory/repos.json`. Beside each control is what
  the project actually uses and which file decided it, and a Shared value that Local overrides
  says so. A value purlis would not read is refused on save, in the words `purlis doctor`
  uses. The old `[memory] share` choice moved into the Shared Plane group, marked as the
  deprecated stand-in for Mode, and it says whether it is in force or a Mode set in either
  file wins. The rest of the old Plane group, `[plane] worktrees` included, is now called
  General.
  ([#300](https://github.com/diazoxide/charter/issues/300))

### Changed

- **purlis lives at `diazoxide/charter`.** The repository that was `diazoxide/charter-app` took
  the name, and the plane that held it before is `diazoxide/charter-plane`. Updates, releases and
  issues come from the new name; a build from before reaches them through GitHub's redirect.
  (ADR 0056)
- **purlis's own plugin is called `charter`.** Its skills are `charter:handoff`,
  `charter:update` and `charter:working-in-a-clone`, and a chat loads it as `charter@inline`.
  It was `charter-app`. A persona whose `skills:` lists a `charter-app:` skill needs it
  renamed, and `purlis persona sync-agents` carries that into `.claude/agents/`. A project or
  workspace setting that still turns `charter-app@inline` on is refused with the new id, and
  the Python charter's `charter@charter` is still turned off in every chat. Turning that one off
  never turns purlis's own off. (#406, ADR 0056)
- **Save in the title bar now saves only the project.** Before, when the workspace in front had
  repos with changes, the title bar's Save became Save all. It committed every changed file in
  those repos and pushed their branches, without asking. Now the title bar counts the repos but
  never saves them. Save all lives only in the Saving tab, and it first asks you to confirm a
  list of each repo, its branch, what it would take and where its save goes. Each repo row says
  where its Save goes, too. A repo nobody has configured is now `off` instead of `pr`: purlis
  saves no repo until `[repos.<name>] mode` says how. To keep saving a repo as before, set its
  mode to `pr`.
- **The project tabs are in the title bar**, after the window controls, and the breadcrumb
  is gone: the project tab says which project and the workspace strip says which workspace.
  That is one tab row fewer above the panes. The tabs give way before About, the update
  button and the ✋ menu do, and a stretch of the bar is always left free to drag the window
  by. How many chats are running is now on the status line.
  ([#394](https://github.com/diazoxide/charter/issues/394))
- **The right sidebar's sections are easier to tell apart.** A line now separates Todos,
  Personas, Vaults and every panel an extension adds. Each heading is a smaller, bolder title
  in brighter text, so it no longer looks like the first row of its list. The left sidebar's
  "Not cloned here" heading matches.
- **`purlis discover` adds to the inventory instead of replacing it.** Engineers on one plane
  reach different repos, and each run used to drop every repo the last person's login could see
  and theirs could not. A repo now leaves `inventory/repos.json` only when `[[forge]].exclude`
  names it. (ADR 0055)
- The alerts drawer no longer repeats what the title bar's save indicator already says about
  the plane: a plane-root alert there now names only a detached HEAD or a branch other than
  the default. Its remedy, in the drawer and on the terminal status line, now reads "save the
  plane, or move the work to a workspace clone".
  ([#332](https://github.com/diazoxide/charter/issues/332))
- `purlis save` follows `[plane] mode`. `off` commits nothing, `commit` stops after the
  commit, and `push` pushes to `[plane] branch` when one is set. Until purlis can open the
  pull request, `pr` and `pr-merge` commit but never push to the target branch. A plane that
  names no mode is saved exactly as before.
- A save with no message says what changed in the plane's own words, for example
  `purlis save: 3 files (steward memory 2, ide todos 1)`, instead of only counting files.
- `[plane] sign = true`, or `--sign`, now signs the save whatever the machine's own
  `commit.gpgsign` says. Before, `--sign` only allowed signing. A signer that fails still
  leaves an unsigned commit, and says so.
- What purlis tells an agent a memory will do, and what `purlis remember` prints, now
  follow `[plane] mode`: a memory travels with the plane's next save. The old text promised
  that `share = "push"` pushed each memory immediately, which this purlis never did.
  ([#293](https://github.com/diazoxide/charter/issues/293))

### Fixed

- **A plane with no workspace offers to make one.** The window drew no way to create the first
  workspace; only the command palette could. The middle of the window now offers "Create a
  workspace", and the workspace strip with its `+` is always drawn.
- **A chat outside every workspace starts in the plane, not in `/`.** With no workspace to start
  in, a chat took the app's own working directory, which is `/` for an app opened from the
  Finder or the Dock, and so also ran without the plane's vault variables stripped.

- A save that deletes a memory file is no longer refused. The secret check asked for the
  deleted file's staged contents, found none, and stopped the save, so making a workspace LOCAL
  could never be saved. ([#301](https://github.com/diazoxide/charter/issues/301))

## [0.2.0] - 2026-09-25

0.2.0 is about settings that belong to a project or a workspace rather than to the machine, and
about vaults you can work with in the window. Project settings and Workspace settings are tabs of
their own, over `charter.toml`, a workspace's `workspace.json` and `charter.local.toml`, and they
choose the extensions, the theme, a workspace's colour and each harness's plugins. A vault can
live in the system keyring and opens in a tab of its own, which reveals or copies a value without
it reaching a chat, and a 1Password token moves into the keyring and out of every chat's
environment. Text size has a Preferences tab, the needs-you queue moves into the title bar, a
handoff is named for its task and can report back, and a relaunch or an update asks before it
reopens your sessions. Repos have right-click menus. The window can invoke only the commands an
allow-list grants it, and a `charter.local.toml` that git would carry no longer decides anything,
and every settings group that it would have changed says so.

### Added

- **Project settings**, a tab of its own: right-click a project's tab and choose _Project
  settings…_, or find it in the palette. It has two sections — **Shared**, `charter.toml`,
  which is committed and your team sees, and **Local**, `charter.local.toml`, which stays on
  this machine — each as a form over the keys purlis documents and as raw TOML for everything
  else. Saving keeps your comments and the order of your keys, and refuses what purlis would
  refuse when it next reads the file, in the same words: a forge it cannot resolve, a profile
  in the committed file, a value that looks like a credential. Local is created on the first
  save, and never where git would commit it. ([#252](https://github.com/diazoxide/charter/issues/252))
- **Extensions per project.** Each project can turn an installed extension on or off, and set
  what it declares, in either section of Project settings: Shared for the team, Local for you,
  and Local wins key by key. The tab shows every extension with what it is in this project —
  on, off, _needs approval here_, or _not installed here_ — and which file decided it. Approval
  stays with this machine: a project that enables an extension you have not approved leaves it
  off until you approve it in Extensions. A project that says nothing keeps every approved
  extension on, as before. Panels, views and themes follow the project in front, and a view
  refuses to run in a project that turned its extension off. ([#253](https://github.com/diazoxide/charter/issues/253))
- **Harness plugins per project.** Project settings has a *Harness plugins* group for each
  harness purlis knows, in Shared and in Local. For Claude Code it lists the plugins installed
  on this machine, and each one can be on, off or not set for the chats purlis starts in the
  project. Local wins plugin by plugin, and not set leaves the plugin to Claude Code's own
  settings. purlis's own plugin is always on and the old `charter@charter` always off. No file
  can change either, and a save that tries is refused. Codex and opencode list what they have
  installed and say their plugins are not supported yet, with the reason: Codex ignores a
  plugin's on/off given for one session, and purlis does not start opencode chats yet.
  ([#274](https://github.com/diazoxide/charter/issues/274))
- **A theme per project.** Project settings has a Theme select in Shared and in Local: purlis's
  dark or light theme, *Follow the system*, or any theme an extension you approved contributes.
  Local wins over Shared. While that project is in front the window and every terminal draw its
  theme, and switching projects switches it live. A pick whose extension is off in the project,
  or not approved on this machine, draws the built-in dark theme, and the tab says why. A
  project's pick wins over your `theme.json`; a project that picks nothing keeps it. ([#273](https://github.com/diazoxide/charter/issues/273))
- **Workspace settings**, a tab of its own for each workspace: right-click a workspace's tab and
  choose _Workspace settings…_, or find it in the palette. A workspace can turn an extension on
  or off and set what it declares, for everyone who works in it: it is kept in the workspace's
  `workspace.json`, committed with a LIVE workspace. It sits between the project's two files —
  `charter.toml`, then the workspace, then `charter.local.toml` — so a workspace refines its
  project and this machine still has the last word, and none of them reaches past this
  machine's approval. Each extension says which of them decided it. The panels and views
  follow the workspace in front, and a view a workspace turned off says so and where. Saving
  changes nothing else in the manifest, and a `workspace.json` from before reads as it always
  did. ([#280](https://github.com/diazoxide/charter/issues/280))
- **A workspace's theme and colour.** Workspace settings has a Theme group: a theme for this
  workspace, over the project's `charter.toml` pick and under your `charter.local.toml` — each
  says which file the theme drawn there came from — and a **colour**: red, orange, yellow,
  green, teal, blue, purple, pink, or one of your own. The colour tints the same theme rather
  than replacing it: the accent and the focus ring while the workspace is in front, its tab and
  its chat strip, and a dot on its tab and in the title bar. Text and the terminal keep the
  theme's colours, so everything stays as readable as the theme was. Every workspace tab shows
  its own colour whether or not it is in front, and switching workspaces switches the theme and
  the tint live — the window's theme now follows the workspace in front, not only the project.
  ([#281](https://github.com/diazoxide/charter/issues/281))
- A workspace can also turn each harness's plugins on or off, in **Workspace settings**: one
  **Harness plugins** group per harness, as in Project settings, with each plugin saying whether
  `charter.toml`, the workspace's `workspace.json` or `charter.local.toml` decided it, or that
  nothing did. A Claude Code chat started in the workspace gets that set; Codex and opencode say
  their plugins are not supported yet, for a workspace as for a project. purlis's own plugin
  stays on and the old one stays off whatever a workspace says.
  ([#282](https://github.com/diazoxide/charter/issues/282))
- `charter.toml` and `charter.local.toml` accept a `[plane]` section and a `[repos.<name>]`
  table for each repo, which say how far a save goes: `mode` (`off`, `commit`, `push`, `pr`
  or `pr-merge`), `branch`, `save_branch`, `sign`, `autosave` and `autosave_after`. The local
  file overrides the shared one key by key. Nothing saves by these settings yet. For now,
  `purlis doctor` and the Project settings tab check them, and the doctor names
  `[memory] share` as the deprecated way of saying `mode`.
  ([#292](https://github.com/diazoxide/charter/issues/292))
- A vault can live in your system's own credential store: the Keychain on macOS, the Secret
  Service on Linux. `purlis vault add <name>` makes one by default, and every `purlis secret`
  and `purlis vault` command works on it as on the other kinds. Each secret is its own
  Keychain item, and on macOS only the purlis program that stored it can read it without the
  Keychain asking you first. A plaintext vault file is now something you ask for, with
  `--provider plain-file`. ([#233](https://github.com/diazoxide/charter/issues/233))
- **Vaults in the app.** A Vaults section in the Attention panel lists each vault with its
  provider and how many secrets it holds. Clicking one opens the vault in a tab of its own, as a
  persona opens, and the tab comes back at the next launch. The tab has a search box, **Add**,
  and a table of name, size and when each secret was written. Each row's menu has Edit value,
  Rename, Copy and Delete. The palette has *Open vault…* and *New vault…*, and a new vault is a
  keyring one unless you pick another kind. Nothing the window lists or writes ever carries a
  value back. ([#234](https://github.com/diazoxide/charter/issues/234),
  [#235](https://github.com/diazoxide/charter/issues/235))
- **Reveal and copy.** A secret's eye shows its value for 30 seconds, until you press it again,
  or until you press Escape. **Copy** puts the value on the clipboard without it reaching the
  window, marked for clipboard histories to skip. purlis clears the clipboard a minute later, or
  when it quits, but only if the clipboard still holds that value. Each reveal and copy writes
  the trace event `purlis secret get --reveal` writes, `secret-reveal`, with a `to` field saying
  `window` or `clipboard`. ([#236](https://github.com/diazoxide/charter/issues/236))
- **1Password tokens go into the Keychain, not your chats.** A 1Password vault's tab has a box
  to paste its service-account token straight into the system keyring; the token never enters
  purlis's own environment. From then on every `purlis secret` command reads it from the keyring,
  so the vault works in a chat and in a terminal that exports nothing. purlis runs only the `op`
  it pinned when the token was stored, verified by path and code-signing team, so a chat cannot
  redirect the token to an `op` of its own; the keyring item is random per vault and machine, and
  the binding it was stored against is pinned locally, so a committed registry change cannot steer
  it. No chat the app starts is given any `OP_*` variable (case insensitively) or any other
  identity variable a vault declares. A tab can also move a token an app was launched with, and
  then warns to relaunch purlis so the export leaves its process.
  ([#237](https://github.com/diazoxide/charter/issues/237))
- **Text size and Preferences.** The window's text and the terminal's each have a size, kept
  per machine, and a change applies at once. Cmd with `=`, `-` or `0` (Ctrl off macOS) makes
  whichever has focus larger, smaller or back to its default; `Ctrl+Shift+-` is left to the
  shell. The defaults are one step larger than before: 14px in the window, 13 in the terminal.
  The sizes live in a **Preferences** tab, which opens from the app menu (`Cmd+,`, or `Ctrl+,`
  off macOS), the palette, and the opener when no project is open.
  ([#283](https://github.com/diazoxide/charter/issues/283))
- An Ignore (✕) on each chat in the needs-you queue takes it out of the queue and out of the red
  counts on its project and workspace tabs at once, without touching the chat. It lasts until
  that chat asks again: its next stop puts it back as a new item. Delete on a focused item does
  the same (Backspace on a Mac), and the palette lists it as "Ignore … until it asks again".
  ([#248](https://github.com/diazoxide/charter/issues/248))
- A launch that has sessions to put back asks first: **Reopen all sessions**, or **Start
  fresh**, naming how many chats and view tabs each project had. Start fresh puts nothing back.
  Escape, closing the question, or no answer at all reopens them, as before.
  ([#250](https://github.com/diazoxide/charter/issues/250))
- When an update is installed, the title bar says **Restart to update**. It restarts purlis
  into the new version and offers every chat and view tab back, with **Reopen all** as the
  answer in front and a line saying purlis restarted to install an update. A chat that is
  mid-turn is named first, and you choose to restart now or wait. If the restart does not come
  back, the next launch offers the same sessions.
  ([#251](https://github.com/diazoxide/charter/issues/251))
- A chat is named for its persona and a number, such as `steward 1`, or for its harness, such as
  `claude 4`, when it has no persona. The picker has an optional Name field, and a chat's tab
  renames by a double-click, F2, its menu's Rename row or the palette. A blank name gives the
  default back. The name is purlis's label only, so a rename never touches the running program,
  and it comes back with the chat after a relaunch.
  ([#254](https://github.com/diazoxide/charter/issues/254))
- A handed-off chat is named for its task. `purlis handoff --name "<short task>"` names the new
  chat's tab, and the handoff skill always writes one from the brief; without it the tab is the
  ordinary `<persona> <N>`, so four handoffs from one chat are four tabs you can tell apart. The
  chat it came from is shown by name, never by number — `↳ from steward 3 · platform-next` in the
  tab's tooltip and the chat's corner, and in the new chat's first line.
  ([#258](https://github.com/diazoxide/charter/issues/258))
- A handoff can ask for an answer. With `purlis handoff --report`, the new chat is told to
  finish with `purlis handoff report "<summary>"`, and the chat that asked gets a needs-you item
  (`<chat> reported back`) and the report as context on its next turn — quoted as data, and never
  typed into it. The report goes only to the chat that asked, and exactly once per handoff —
  another needs another `--report` handoff; if that chat has closed, the next chat in its workspace learns it when
  it starts. Without `--report`, nothing changes. ([#259](https://github.com/diazoxide/charter/issues/259))
- Right-click a repo — its heading in the explorer, or its row in the bottom bar — for **New tab
  in** it, which starts that one tab's chat in the clone, and **Start new chats in** it, which
  makes the clone where every new chat starts until you pick somewhere else, as picking a
  worktree does one level down, and the explorer marks it.
  Shift+F10 or the menu key opens any of purlis's menus on the row the keyboard is on.
  ([#174](https://github.com/diazoxide/charter/issues/174))
- The explorer is a tree to a screen reader and to the keyboard: Right opens a clone or moves to
  a row's first child, Left closes it or moves to its parent, and a typed letter moves to the
  next row starting with it. ([#238](https://github.com/diazoxide/charter/issues/238))
- Delete on a focused project or chat tab closes it, as its × does, and so does Backspace on a
  Mac. Ending a chat still asks first, and closing a project that has chats open now asks too,
  from the ×, Delete, the tab's menu and the palette, naming each chat it would end.
  ([#239](https://github.com/diazoxide/charter/issues/239))

### Changed

- The needs-you queue is in the title bar now, and nowhere else. A hand and a count sit left of
  About when anything needs you. When nothing has asked but a chat that can't report is open — a
  shell, or a harness without purlis's hooks — it is a faint hand with no number, and its
  tooltip and list name those chats ("shell 2 can't tell purlis it's waiting"). With neither,
  nothing is there. Pressing it lists every
  chat asking in every open project — its name, then its workspace and project — each with
  **Go**, which brings that chat to the front and switches project and workspace to get there,
  and **✕**, which ignores it. The Attention panel no longer has the queue; its other sections
  are unchanged. From the keyboard, Tab reaches the button, Enter opens the list, the arrows
  move, Delete ignores, and Escape closes it.
  ([#249](https://github.com/diazoxide/charter/issues/249))
- Tab reaches the whole window, in the order it is drawn. Each strip and each list is one stop,
  and the arrows, Home and End move inside it. A terminal keeps Tab for its shell, and
  Ctrl+Tab and Ctrl+Shift+Tab leave it. A pane's split and close controls show on hover and when
  the keyboard is on them, not all the time on the pane you are typing in.
  ([#189](https://github.com/diazoxide/charter/issues/189))
- Nothing in the window rubber-bands on macOS any more. A panel scrolls and the window does not,
  and a scroll no longer carries out of a panel into the page.
  ([#263](https://github.com/diazoxide/charter/pull/263))

### Fixed

- On Linux and Windows the app menu no longer takes a key the chat's shell owns: `Ctrl-C` in a
  chat is the interrupt again, not Copy, and the same goes for `Ctrl-A`, `Ctrl-Z`, `Ctrl-Y`,
  `Ctrl-V`, `Ctrl-X` and `Ctrl-H`. Quit is `Ctrl+Shift+Q` there, as in a terminal app. macOS is
  unchanged. ([#241](https://github.com/diazoxide/charter/pull/241))
- `purlis doctor`'s `git auth` row checks the one-credential git policy, the check
  `purlis git-policy` runs, instead of saying it is not checked. It only reads, and names
  `purlis git-policy --apply` for a clone that drifted.
  ([#241](https://github.com/diazoxide/charter/pull/241))
- Closing a chat that was asking for you, with the × on its tab or by ending its pane, takes it
  out of the needs-you queue and out of the red counts on its project and workspace tabs. It
  used to stay there until some other chat moved.
  ([#247](https://github.com/diazoxide/charter/issues/247))
- A chat's report that raced a close, or an Ignore, can no longer put the chat back in the
  needs-you queue: every update the window gets is numbered, and it keeps the newest.
  ([#248](https://github.com/diazoxide/charter/issues/248))
- A persona's card names the vault `purlis persona list` names. A persona whose definition has
  no `vault:` line but that `vaults.json` tags a vault to used to be shown as "not declared in
  its definition"; the card now shows that vault's name and says it came from the vault
  registry. A persona nothing names a vault for says so, a `vault: none` still says it holds no
  credentials of its own, and a registry that does not read is shown with purlis's reason.
  ([#185](https://github.com/diazoxide/charter/issues/185))
- `purlis persona stats` reads a dispatch log's timestamps as Python's
  `datetime.fromisoformat` did, digit for digit. A stamp such as `2026-03-04T100`, with three
  digits for the time, is skipped rather than read as ten o'clock, and so is a one-digit hour,
  minute or second. Any one character between the date and the time, a comma before the
  fraction and an offset with seconds all read as Python read them.
  ([#315](https://github.com/diazoxide/charter/issues/315))
- The panels follow the plane on disk. A todo closed with `purlis ws todo done` in a terminal
  leaves the Todos panel and its count at once, and a workspace made in a terminal is watched
  from then on. Before, a panel changed only when you focused another workspace and came back.
  ([#264](https://github.com/diazoxide/charter/issues/264))
- `purlis save` against a remote that moved no longer waits on a signer that never answers.
  The rebase that replays its commit has a two-minute deadline, as the commit itself does, and a
  rebase stopped at it is reported as out of time, not as a conflict. `purlis save --sign`
  replays its commit signed, and a save without `--sign` never asks a signer.
  ([#242](https://github.com/diazoxide/charter/issues/242))

### Security

- The window can only invoke the commands on the app's allow-list. Every command it calls is
  now listed in one place and granted to the main window by name. Anything not on the list is
  refused before it runs, and so is a call from any other window. A vault's reveal and copy
  have a grant of their own and reach the main window only, so a window added later does not
  get them by default. The Content-Security-Policy is tighter as well: the window loads no
  plugins or frames, submits no forms, and accepts no `<base>`. The policy and the allow-list are
  now separate guards on reveal and copy. Before, the policy was the only one.
  ([#276](https://github.com/diazoxide/charter/issues/276))
- A `charter.local.toml` that git tracks, or would commit, no longer decides anything. The file
  is meant to stay on one machine, and purlis already refused the harness profiles in it when
  git would carry it. The extensions, theme and harness plugins it chose were still applied,
  though, so a copy committed by mistake reached every clone of the plane. Now purlis reads
  nothing in such a file, and the workspace and `charter.toml` decide instead. The Local section
  of Project settings still shows the file and says why it is not read and how to fix it: add
  `/charter.local.toml` to `.gitignore` (`purlis reinit` does that), or, if git already tracks
  it, `git rm --cached` it first. ([#308](https://github.com/diazoxide/charter/issues/308))
  The Extensions, Theme and Harness plugins groups say it too, in the same words, in Project
  settings and in every Workspace settings tab, wherever the file set something. Before, a
  value you set in Local showed as decided by `charter.toml` or the workspace, with no reason
  given.
  ([#319](https://github.com/diazoxide/charter/issues/319))

## [0.1.1] - 2026-09-24

0.1.1 brings back what 0.1.0 left out and a working plane still used: vault access through
`purlis secret` and `purlis persona secret`, and the `persona use`, `list`, `sync-agents` and
`stats` commands. A chat started by charter 0.1.0 finds the app's own `charter` first on its
`PATH`, so a plane whose instructions call those commands lost them. This release restores them.

### Fixed

- `purlis persona list`, `persona use`, `persona sync-agents` and `persona stats` work again,
  and answer as the purlis your plane was set up with did. Re-syncing a plane's sub-agents
  changes only the ones whose persona changed since they were last generated.
  ([#228](https://github.com/diazoxide/charter/pull/228))
- `purlis ws todo` says what it recorded, closed or dropped, and a slug that is not there
  says so instead of passing silently. ([#228](https://github.com/diazoxide/charter/pull/228))
- `purlis secret`, `purlis persona secret` and `purlis vault` are back. A chat that runs
  `purlis secret exec <vault> --file KUBECONFIG=<key> -- kubectl …` or `purlis secret list
<vault>` got a usage error from 0.1.0, which put charter first on the chat's `PATH` without
  them; they now answer as the Python charter did, with the plain-file, reference and 1Password
  providers, and a value still never reaches the chat: `list` prints names, `get` a size band
  and a keyed fingerprint, and `exec` hands values to the command's environment or to 0600 temp
  files it removes, redacting what the command prints.
  ([#227](https://github.com/diazoxide/charter/pull/227))
- The Bash guard refuses a vault file read that is wrapped in `purlis secret exec … --`, the
  way it refuses one wrapped in `env`. ([#227](https://github.com/diazoxide/charter/pull/227))
- The Bash guard no longer mistakes text for a handoff. A multi-line quoted string that
  mentions `purlis handoff`, such as a commit message, is read as the text it is, and a real
  `purlis handoff` after it is still judged. ([#226](https://github.com/diazoxide/charter/pull/226))
- A harness profile that wraps another program (`["ccs", "work"]`) starts as
  `ccs work --plugin-dir …`, with purlis's flags after the profile's own words, so a wrapper
  that expects its subcommand first works. A plain `claude` or `codex` profile starts exactly as
  before. ([#226](https://github.com/diazoxide/charter/pull/226))
- What `purlis docs show` serves, and every message purlis prints, name only commands this
  purlis has. A page about something it does not do is gone, and a planned command says "not in
  this version yet". ([#226](https://github.com/diazoxide/charter/pull/226))

## [0.1.0] - 2026-09-23

### Added

- purlis is a desktop app for macOS and Linux. A window holds your projects as tabs, each
  project's workspaces, and each workspace's chats, and a chat is a live terminal running
  Claude Code or Codex.
  ([#14](https://github.com/diazoxide/charter/pull/14),
  [#111](https://github.com/diazoxide/charter/pull/111),
  [#125](https://github.com/diazoxide/charter/pull/125),
  [#131](https://github.com/diazoxide/charter/pull/131))
- Opening a project that you have not approved shows what it would run first, and nothing runs
  until you say yes. ([#110](https://github.com/diazoxide/charter/pull/110),
  [#121](https://github.com/diazoxide/charter/pull/121),
  [#145](https://github.com/diazoxide/charter/pull/145))
- A new chat starts on the harness profile and persona you pick, in the workspace or in one of
  its worktrees. ([#32](https://github.com/diazoxide/charter/pull/32),
  [#41](https://github.com/diazoxide/charter/pull/41))
- Every chat says what it is doing, and the chats waiting for you are listed first and counted
  in the window. ([#26](https://github.com/diazoxide/charter/pull/26),
  [#51](https://github.com/diazoxide/charter/pull/51),
  [#157](https://github.com/diazoxide/charter/pull/157))
- Chats open as tabs and split side by side. The split and close buttons sit on the pane they
  act on, and ending a chat asks first. ([#14](https://github.com/diazoxide/charter/pull/14),
  [#176](https://github.com/diazoxide/charter/pull/176))
- The window has four regions: a worktree explorer on the left, the chats in the centre, and a
  bottom bar with each repository's branch, changes, worktrees and running pipeline.
  ([#141](https://github.com/diazoxide/charter/pull/141),
  [#154](https://github.com/diazoxide/charter/pull/154),
  [#156](https://github.com/diazoxide/charter/pull/156))
- A worktree can be cut and removed from the window, and a workspace or a project can be made
  and deleted there too. ([#31](https://github.com/diazoxide/charter/pull/31),
  [#34](https://github.com/diazoxide/charter/pull/34),
  [#172](https://github.com/diazoxide/charter/pull/172),
  [#192](https://github.com/diazoxide/charter/pull/192))

  *Erratum:* 0.1.0 could remove a worktree from the window but not cut one, and no later
  version can yet. Cut one with `purlis worktree add` (since 0.4.0); cutting from the window
  is [#701](https://github.com/diazoxide/charter/issues/701).
- A command palette and right-click menus reach every action the bars have.
  ([#45](https://github.com/diazoxide/charter/pull/45),
  [#172](https://github.com/diazoxide/charter/pull/172),
  [#194](https://github.com/diazoxide/charter/pull/194))
- A project, a workspace and a chat can each be pinned.
  ([#143](https://github.com/diazoxide/charter/pull/143))
- A status line along the bottom of the window carries the doctor, the alerts drawer for every
  open project, and a note when a project pins an older purlis.
  ([#153](https://github.com/diazoxide/charter/pull/153),
  [#160](https://github.com/diazoxide/charter/pull/160),
  [#162](https://github.com/diazoxide/charter/pull/162),
  [#165](https://github.com/diazoxide/charter/pull/165))
- Each chat shows its context and cache gauge in its pane's corner, with a bar per turn for its
  usage trend. ([#164](https://github.com/diazoxide/charter/pull/164),
  [#167](https://github.com/diazoxide/charter/pull/167))
- The personas panel lists each persona with its memories, searchable and loaded a page at a
  time. ([#173](https://github.com/diazoxide/charter/pull/173),
  [#206](https://github.com/diazoxide/charter/pull/206))
- An extension is a directory you point purlis at. Nothing it declares is in force until you
  approve it, and purlis asks again when anything in that directory changes.
  ([#150](https://github.com/diazoxide/charter/pull/150),
  [#180](https://github.com/diazoxide/charter/pull/180))
- A request that belongs in another chat can be handed off from inside the app.
  ([#207](https://github.com/diazoxide/charter/pull/207))
- The title bar says which project, workspace and chat you are in, and opens About purlis.
  ([#205](https://github.com/diazoxide/charter/pull/205))
- The app updates itself from a stable or a dev channel, and installs only what the release
  key signed. ([#158](https://github.com/diazoxide/charter/pull/158))
- The `charter` command ships inside the app, so hooks and the Bash guard answer without a
  Python install. ([#168](https://github.com/diazoxide/charter/pull/168),
  [#181](https://github.com/diazoxide/charter/pull/181))
- A tab can hold a view, not only a chat. A persona opens as its own tab: what it is for, its
  tools and vault, and its memories, searchable. An approved extension can add a view of its
  own. The first is persona statistics, with charts of each persona's memories, which purlis
  asks one question at a time and only when you open it. ([#212](https://github.com/diazoxide/charter/pull/212))
- The view tabs you had open come back at the next launch, and wait for a click before an
  extension is asked anything.
  ([#212](https://github.com/diazoxide/charter/pull/212))
- The palette can put the app's own `charter` on your terminal's `PATH` on macOS: **Install
  `charter` command in PATH** links `/usr/local/bin/charter` to it, and never replaces a
  `charter` something else put there. ([#219](https://github.com/diazoxide/charter/pull/219))
- A chat opens knowing who it is: the persona you picked, what it remembers, and the
  workspace's todos arrive with its first message. The guards on reading a vault, on writing
  into purlis's own state, and on sending a sub-agent run in the app's own `charter`.
  ([#220](https://github.com/diazoxide/charter/pull/220))

### Changed

- A chat needs nothing installed from the Python charter. The app carries its own Claude Code
  plugin with purlis's hooks, its Bash guard and its skills, and loads it into each chat it
  starts, for that chat alone. The chat turns the Python charter's plugin off for itself, and
  finds the app's own `charter` first on its `PATH`. A chat starts offline.
  ([#219](https://github.com/diazoxide/charter/pull/219))
- The light and dark themes are data files, and the window and the terminal are both drawn from
  them. ([#144](https://github.com/diazoxide/charter/pull/144))
- The terminal follows a theme switch while it is open. The window's layout lives in
  `charter/layout.json`, which you can edit by hand, and it is in place before the first
  frame is drawn. ([#215](https://github.com/diazoxide/charter/pull/215))
- About purlis tells this app's own story: its version and what that version brought.
  ([#215](https://github.com/diazoxide/charter/pull/215))
- The region toggles sit at the left end of the status line. The context gauge floats over its
  pane rather than taking a row from it, and a very light line divides one tab from the next.
  ([#214](https://github.com/diazoxide/charter/pull/214))
- The project, workspace and chat strips nest, and tabs that do not fit collapse into a
  _N more_ button instead of scrolling. ([#139](https://github.com/diazoxide/charter/pull/139),
  [#171](https://github.com/diazoxide/charter/pull/171))
- Closing the window hides it to the tray. Quitting says which chats it will end, and the next
  launch puts back the projects and chats you had open.
  ([#23](https://github.com/diazoxide/charter/pull/23),
  [#125](https://github.com/diazoxide/charter/pull/125))
- Tab moves through every dialog, and Ctrl-K belongs to the chat that has the keyboard.
  ([#188](https://github.com/diazoxide/charter/pull/188),
  [#190](https://github.com/diazoxide/charter/pull/190))
- `purlis init` adopts the repository it is pointed at instead of turning it into a plane.
  ([#115](https://github.com/diazoxide/charter/pull/115),
  [#197](https://github.com/diazoxide/charter/pull/197))
- The app, the dock and the menu bar carry purlis's own mark.
  ([#159](https://github.com/diazoxide/charter/pull/159),
  [#203](https://github.com/diazoxide/charter/pull/203))
- A macOS build is ad-hoc signed when no Apple Developer ID is set up. The first install needs
  one command, and the release page says which.
  ([#201](https://github.com/diazoxide/charter/pull/201))
- `purlis version` prints the app's own version. A plane pinned to a release
  of the Python charter is reported as that older line, not as drift. `purlis doctor` and
  every other message stop sending you to the Python charter, and `purlis docs show`
  describes this app. ([#219](https://github.com/diazoxide/charter/pull/219),
  [#223](https://github.com/diazoxide/charter/pull/223))

### Fixed

- A program that starts a screen update and never finishes it no longer freezes the pane.
  ([#7](https://github.com/diazoxide/charter/pull/7),
  [#19](https://github.com/diazoxide/charter/pull/19))
- A pane opened late catches up on what the chat already printed.
  ([#11](https://github.com/diazoxide/charter/pull/11))
- A chat started from an app opened in Finder finds `charter` and its harness.
  ([#135](https://github.com/diazoxide/charter/pull/135),
  [#168](https://github.com/diazoxide/charter/pull/168))
- An extension's program that crashes is reported with its exit status and its last words,
  not as a lost connection. ([#217](https://github.com/diazoxide/charter/pull/217))
- A slow `git` is no longer reported as a broken repository.
  ([#44](https://github.com/diazoxide/charter/pull/44))
- No program purlis starts can hold a chat's terminal open after the chat ends.
  ([#105](https://github.com/diazoxide/charter/pull/105))

[Unreleased]: https://github.com/purlis/purlis/compare/v0.5.1...HEAD
[0.5.1]: https://github.com/purlis/purlis/releases/tag/v0.5.1
[0.5.0]: https://github.com/purlis/purlis/releases/tag/v0.5.0
[0.4.1]: https://github.com/diazoxide/charter/releases/tag/v0.4.1
[0.4.0]: https://github.com/diazoxide/charter/releases/tag/v0.4.0
[0.3.0]: https://github.com/diazoxide/charter/releases/tag/v0.3.0
[0.2.0]: https://github.com/diazoxide/charter/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/diazoxide/charter/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/diazoxide/charter/releases/tag/v0.1.0
