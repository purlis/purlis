# Every change in 0.5.1

The complete list of what [0.5.1](../CHANGELOG.md) brought, from v0.5.0. The changelog's
0.5.1 section has the highlights.

## Added

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

## Changed

- **What purlis tells a chat says a chat is in a pane, not a tab.** The session record tool, the
  smart-close skill and `purlis session record` say the chat ends, not that its tab closes. A
  question a chat raises for you is said to wait in the Inbox, and on the chat's pane when it is
  on screen. The secrets, safe-remove and theme skills, the docs `purlis docs show` serves, the
  getting-started pages, the README and the site's home page say the same (#4770).

- **Dragging over a folded pane says why it takes no drop.** The words by the pointer now say a folded pane takes no drop, and letting go there cancels, with the no-drop cursor (#5415).

- **A large project opens without waiting for its watch on macOS.** The project's one FSEvents
  stream starts first, and the folders whose changes reach the window are listed after it, on the
  watch's own thread. On a project of 1,000 workspaces that list is about 5,000 folders, and the
  open waited 0.8 to 2.2 s for it on a busy machine. A change made in the meantime is held
  until the list is ready, so none is lost (#5672).

- A browser login a chat suggests marks the part of each host its owner registered, so a known
  name borrowed on its left, or a swapped letter, stands out.

- **A browser login a chat suggests on a page other than the site's front page says so.** The
  notice shows that page on its own line, as large as the site, and warns that a page can sign the
  browser in to someone else's account as it opens, a usual sign-in address too. A site typed over
  it is drawn again only for a warning the notice hadn't shown (the person's ruling of 2026-11-05).

- **A chat on a runner starts only once its profile's harness settings file can be read.** When
  Claude Code's `settings.json`, Codex's `config.toml` or opencode's `opencode.json` is a link, a
  file with a second name on the disk, or can't be opened, the chat is not started there and its
  tab says which file and what to do: keep the file itself in the settings folder, under that one
  name. A link elsewhere in the settings, such as an agent's file, is still left out and said.

- A browser login notice from a chat on a pool's runner goes after 15 minutes with no answer, as
  one from a chat on this machine does, and the chat is told nobody answered.

- `purlis worktree update` standing in a workspace's clone, for a sandboxed chat, no longer
  rebases there in the app. The app fetches, works out the rebase and names the exact `git`
  command for the chat to run in the clone; `purlis worktree update --continue`, or the update
  run again, then puts the branch on the result. `--abort` before the named step has run puts
  the folder back as it was. Branch folders, chats that are not sandboxed and the terminal are
  unchanged.

- **Sending a task back says it goes back to its row.** The − on a task's tab or pane, its rows
  in the palette and the menus, and its line in a tab's chip all say "Send talk back to its
  row", not "back into steward 1's tab", "back among steward 1's tasks" or "back into this tab"
  (#5667).

- **Sending a task back says it stays in the Chats list.** The note on a task's − says nothing
  ends and its row in the Chats list stays, not that its session's tab lists it. The notes on
  moving a task to a tab of its own and opening it beside a chat no longer speak of a session's
  tab either (#5664).

- **The window says "shell", not "shell tab".** A shell is shown in a pane, as a chat is, so the
  menus say *Open a shell here* and *Open a shell in branch …*, and Install and a harness's
  sign-in run "in a shell". A forge's login is typed "in a pane", and a terminal editor opens in
  a pane beside the one you are in (#5637).

- **A sign-in lets the chat act on its provider's site only in the sign-in's own window.** While
  `browser_login` signs in on another site, the chat reads and acts there on the page the login
  ran on and the windows it opens; another tab stays kept off that site.

- **A browser holding a login takes a secret only for the login's own sites.** A chat's fill on
  any other site is refused before anything is filled, and the chat is told to use a browser of
  its own, where it once left the browser reaching nothing.

- **A sign-in that asks for a second secret on its provider's site finishes by itself.** Once
  that site's server sends the page back to the login's own site, the page goes there without
  asking, while the sign-in is open.

- **A file a blank or embedded frame made is said to come from the site around it.** In a browser
  that signs in on another site, one from a frame with no site of its own is brought into the
  chat's folder only during a sign-in.

- **A skill you have twice is listed once.** When a project skill is the same, file for file, as
  one you installed yourself or through a plugin, a chat lists only your copy and the project's
  copy says which one was kept. When the two copies differ, the chat keeps both and its Skills
  view names the pair, so you choose which one to keep (#5737).

- **A split tab keeps its first chat's name, shows the pane that needs you, and pictures its
  layout.** Splitting a tab or moving between its panes no longer renames it. When its first chat
  closes, it takes the next one's name. Its mark is its most urgent pane's, so a pane that needs
  you shows on the tab while another pane is focused, and a task shown in a pane counts too. In
  place of `+1`, a small picture after its name shows the layout's shape, and hovering says how
  many panes it holds and lists them (#5342).

- **A tab's name has room.** A tab shrinks no further than 160px, about fifteen letters of its
  name, before the rest go behind *N more*. Its × takes no room: it shows over the end of the
  name on the tab in front, under the pointer and with the keyboard on the tab, and the front
  tab's × is reached with Tab. A tab no longer counts its tasks: press the state at the top of
  the chat's pane ("waiting on 3 tasks"), or ⌘⇧J, for the same menu, and a tab's hover says how
  many tasks are running (#5341).

- **The title bar's crumb and a switcher row take a control's corner.** They draw
  `radius.control`, as a workspace chip does, instead of a step above `radius.md`. Purlis and
  Purlis Classic look the same as before. A theme that extends neither draws them at its
  control's corner, or sets `titlebar.crumb.corner-radius` and `titlebar.row.corner-radius`
  (#3196).

- **A `purlis` command that waits on you says where to answer.** `purlis secret ask`, a Secret
  ask from a brokered command, a `purlis secret exec` run that needs your Allow, and a runner
  chat's tool run now print "waiting for the person: Asks (✋) in the purlis window, project
  <name>, for up to N seconds". When the command stops waiting, it says the ask is still
  answerable there.

- **A chat's browser act lands only where its element is drawn.** A click, hover or drag on an
  element that another element or frame is drawn over is refused as not shown, and nothing is
  pressed. A click still lands through a box the pointer passes through, and on an element whose
  label, label's `::before`, or a box around it (a web component's around its slot included) is
  drawn over it. A click reads again what is drawn at its point once the mouse is there, and an
  act on a page's own document waits for its scroll to be drawn before it is aimed.

- **An older runner of a pool stays while it holds work purlis can't bring back on its own.**
  Before it goes after an update, purlis asks it for every repository it holds and checks each
  against the project's folder here, as it already does for a task's own runner. A stash, a
  commit on no branch, a clone made only on the runner, or one whose clone or workspace was
  removed or renamed here keeps it, and so does work of a project this machine no longer knows.
  Settings › Pools lists what keeps it, and says how to keep that work or let it go.

- **An older runner purlis keeps unchecked now says so in plain words.** Settings › Pools gives
  each one a line of its own, by name, and Devices and Let it go say what is left of it: a
  stopped container, which purlis checks once you start it, or only its saved home, which purlis
  can't check. Letting it go deletes whatever it holds.

- **An ask is answered in its row, behind an Answer link, and a result opens in place.** An ask
  the window can answer (a reply, a permission, a task's question) shows Answer, which opens its
  reply box or its options in the row with the keyboard in them, and reads Cancel while it is
  open; Enter in the box sends. A task's question is answered there too, with no dialog. A prompt
  in a chat's terminal and a browser waiting for a sign-in show Go to chat alone. A line under
  Mine has Read, which opens its report or the reason it failed in place and marks it read
  (ADR 0119, #5651).

- **Asks and Updates are walked as lists of rows.** ↑ and ↓ now move from one ask (or update)
  to the next, and ← and → move through an ask's own links, answers and reply box, where ↓ used
  to step through every button of an ask before the next. Home and End go to the first and last
  ask. A reply or reason box keeps its own arrows, Home, End and Space, and Esc in it goes back to
  its ask. Updates is its own list after Mark all read and Dismiss all, reached with Tab
  (#3123).

- **Asks heads the right sidebar, where the Inbox was.** It lists every ask, grouped by chat with
  the longest wait first, the scope switch, and the chats waiting in other windows; its hand
  counts the asks. The ✋ and ⌘⇧I (Ctrl+Shift+I) open it, and a layout that kept the Inbox opens
  on it. The Notices and the updates are in the Updates view after it, and a new ask never
  changes the view that is open (ADR 0119, #5644).

- **A chat in a branch folder leaves the clone's git to the app.** A Claude Code chat started in a
  branch folder purlis cut no longer writes its clone's `.git`, as a chat above the folder already
  did: `git fetch`, `rebase`, `reset`, `stash`, `add` and `commit` stop there, and read-only git
  works as before. `purlis worktree commit`, `update` and `push` do those, and the new `purlis
  worktree update --fetch-only` fetches and moves nothing else, also with uncommitted changes.

- **A rebase stopped in a clone goes on or back by the chat's own git.** A sandboxed chat is told
  the step, `git -c core.editor=: rebase --continue` or `git rebase --abort`, and `purlis worktree
  update --continue` or `--abort` then puts the branch where it belongs. A branch folder and your
  own terminal work as before.

- **The app keeps its own record of an update in progress**, under the project's state, not in a
  git directory. A rebase stopped before you updated purlis, or a rewritten branch not yet pushed,
  is finished or pushed from your own terminal.

- **`purlis worktree push` asks the remote what it has**, and scans what the push sends against
  that, not against the clone's remote-tracking branches. The app's git reads commits as they are,
  without `refs/replace/`.

- **Chats' browsers run Chrome for Testing 157.0.8103.38**, up from 155.0.8059.79. The engine
  this build ships with is current again, so its browsers start before purlis has read a newer
  engine manifest.

- **purlis's built-in skills, a chat's briefing and purlis's tool descriptions use one
  vocabulary.** They say the person (never the user or the operator), helper (never sub-agent),
  branch folder, repo and chat, as `CONTEXT.md` does, and the shipped-skill check fails on
  "control plane", "sub-agent", "the user" and "operator" outside a tool's name, and on a skill
  named in prose that purlis does not ship (#5723).

- **opencode chats list your plane's skills beside purlis's own.** purlis's built-in skills' descriptions are shorter, together at most 5,000 characters, and opencode's default skill budget is 8,000 characters, so the built-ins no longer fill it alone (spec #5714).

- **Codex chats list your plane's skills beside purlis's own.** Codex's default skill budget is 8,000 characters, within what Codex keeps of the hook that lists them, so purlis's built-in skills no longer fill it alone (spec #5714). Each folder of skills is named once in a Codex chat's skills list, not on every skill's line, so purlis's own leave about 3,000 characters for yours wherever purlis is installed.

- **A chat starts faster on a project with many personas.** Freezing every persona's tools for
  the new chat now happens while its briefing is put together, not before it, and is still
  finished, and written to disk, before the chat's first turn. On the 100-persona test project a
  chat start went from 68 ms to 47 ms on a Mac (#5486).

- A new chat's briefing reads the project index the session host keeps, so a chat starts quickly on a large project. The memory counts, the oldest todos and the other workspaces come from the index, and the unshared-memory line reads what the host last found, with no `git status` of its own. While the host builds a project's first index, the briefing says so and leaves out the other workspaces, the memory counts and the open todos.

- A chat says one of eight states, with one icon and one colour each, the same on its row in the Chats list, its tab, its pane's head and in Asks: Working, Waiting on you, Waiting on a chat, Stuck?, Done, Failed, Idle and Can't tell. A dispatched task whose turn ended now reads Waiting on a chat instead of idle, a chat waiting on its own background work reads Working, and a shell reads Can't tell (#6012).

- **The Chats list works like every other tree in the window** (#3198). A chat's browsers, the
  tasks it asked for and its finished tasks are now rows under it, so the arrow keys reach them
  all and folding the chat folds them too. Finished tasks that came out done fold into one
  **Finished (n)** row with **Clear finished**. → moves from a row onto its own buttons: a
  finished task's Brief, Reopen and Clear, the hand that goes to a chat below that needs you,
  and the **Dispatch settings** link of a chat at its task limit, which the keyboard could not
  reach before. Tab comes back to the row the keyboard was last on, as it does in the Explorer.

- **A large project's saving status costs a fraction of the git work it did.** Where your git
  supports them, a save leaves git's untracked cache and, on macOS and Windows, git's own file
  system monitor in the project's index, so the title bar, the Saving tab, the alerts and
  auto-save read the project without opening every folder or checking every file. On macOS the
  project's one watch tells purlis when anything moved, so an idle window no longer reads git
  every ten seconds. With an older git, purlis reads the project as before.

- **A Claude Code chat on Linux under Claude Code's own sandbox hands a vault's value to a command
  by file.** `purlis secret exec --env` is refused there with a sentence naming `--file`, which the command
  reads the value from instead. Declare a persona's tool `as = "NAME file"` for such a chat.

- **purlis runs Claude Code chats on Linux inside its own sandbox, on a profile for sandboxed chats
  only.** Such a chat runs whole inside purlis's Linux wrap, as Codex and opencode chats do and as
  Claude Code chats on a runner already did, and needs no bubblewrap or socat. Its project lists its
  model's hosts at the full sandbox (`model-providers`). A Claude Code chat on a profile for every
  chat keeps Claude Code's own sandbox.

- **Claude Code keeps the descriptions of your own skills and plugins.** The listing limit purlis
  hands a Claude Code chat now holds the skills and plugins you installed yourself, up to Claude
  Code's own ceiling. Past it, the chat's Skills view says some descriptions are dropped and names
  what takes the most room. On Codex, purlis's built-in skills are listed first, so Codex never
  drops one of them (#5736).

- **More of purlis's own failures exit 70 and are drafted as a Report.** `purlis workspace list`
  and `workspace optimize` where `workspaces/` cannot be listed, `workspace vision` where
  `workspace.md` cannot be written, `purlis skill edit` where its editing copy cannot be made or
  read back, and `purlis dispatch` and `purlis report` where the app or the Reports module answer
  something the command did not ask for now exit 70, the code for a failure inside purlis, in
  place of 1. purlis's tools for dispatched tasks mark the same failure in their error. A refusal
  and a usage error still exit 1.

- **A clone in a workspace's `repos/` folder is seen wherever a clone at its root is.** The repo
  list, Explorer's repo rows, the Panel's Repos view, `purlis doctor`'s clone checks, the
  workspace layer's exclude block, the sandbox's protection of each clone's git folder, a chat's
  repo skills and the code sent to runners all find it there. Where a workspace holds a repo both
  in `repos/` and at its root under one name, the one in `repos/` is the clone (ADR 0114).

- **A wide workspace's Explorer no longer reads every repo's status to count its changes.** The
  closed repo rows' change counts are one ask of the core, which keeps each repo's count until
  something in that repo moves, so a workspace of twenty repos no longer reads twenty statuses on
  every refresh (#3489).

- **A decision is answered on its row in Asks.** A permission, a run grant, a sandbox's host,
  folder or service, a browser's site and a dispatch now show their options on the row, with the
  question as the title ("Allow curl to use vault team?"), laid out as a chat's band lays them
  out: the narrowest Allow, **Other scopes…** for the wider ones, and the refusal. "Why keep it
  blocked?" opens only after you press Keep blocked. An ask that needs typing, a reply or a
  secret's value, keeps its form behind a visible **Answer** button, and its row says what it
  needs.

- **Finding duplicate memories stays quick on a large store.** `purlis persona dedupe`,
  `workspace optimize`, `persona optimize` and `persona stats` compare each memory only with the
  memories that share one of its rarer words, instead of with every other memory. They find
  exactly the pairs they found before, and a store of 10,000 memories takes seconds instead of
  minutes.

- A project that has run thousands of tasks no longer reads every past dispatch to draw the
  sidebar's task status or each turn's list of open tasks. Ended dispatches are summarised in
  one small file in the dispatch store (`app/dispatches/ended.json`), and only running records
  are read in full. Deleting the file is safe: the next read rebuilds it (#5502).

- **Every chat on a runner starts inside purlis's sandbox.** A Codex, opencode or declared
  harness's chat there that would start without it is refused in words, as a Claude Code chat
  already was, and a runner made before purlis 0.5.0 carries no sign-ins to its pool's next
  runner: those profiles are signed in again once.

- **A sign-in in a shell on a pool's runner lets a drain carry it again**, as the driven sign-in
  does, and a sign-out ends the watch of that sign-in.

- **Every browser login starts clean on its own site.** A login that fills first clears what that site
  stored in the browser: cookies, service workers, local storage, IndexedDB and cache, however the
  login was allowed. Other sites are left as they are, and a browser that already holds the login
  answers "already logged in" and clears nothing. If the login form doesn't appear after that, purlis
  opens a clean new browser for the login, says why in the answer and in the browser's timeline, and
  offers to close the one left behind (#5272).

- **With the workspace picked, Explorer shows the workspace's folder.** Its own files sit beside
  its repos, and each repo's row is where the repo is on disk: in `repos/`, or at the top for a
  repo cloned before. One repo still opens at once. What purlis keeps there is marked *managed*
  and opens read-only, with a link to its own view (Memory, Todos, Sessions, or the vision for
  `workspace.md`). Dotfiles show with **Show hidden**. The filter finds among the workspace's
  files, and with one repo it covers those files and the repo (#5375).

- **A skill from an extension's template waits in Drafts.** With the Skill studio on, New skill…
  from a template an extension offers, or `purlis skill new --extension-template`, keeps the skill
  as a draft to try, then accept or discard, instead of making it at once. purlis's own templates
  still make the skill in one commit (#3091).

- **Five built-in skills say when they are for, and which skill to use instead.** The
  descriptions of `purlis-secrets`, `purlis-handoff`, `purlis-smart-close`, `purlis-safe-remove`
  and `purlis-persona` now open with the situation in the words people use and end with a "Not
  for … — that is purlis-…" line for each skill they are confused with. This helps your chat
  program pick the right one, for example a handoff when you will read the new chat yourself and
  a dispatch when this chat needs the answer. Each now has trigger tests, at the strict bar.

- **Four built-in skills have names that say what they do.** `purlis-compact` is now
  `purlis-tidy-memory` (it no longer looks like Claude Code's `/compact`),
  `purlis-working-in-a-clone` is `purlis-repo-work`, `purlis-update` is `purlis-upgrade`, and
  `purlis-add-curation-action` is `purlis-curation-action`. A persona that lists an old name
  still gets the skill, and the runs, grades and misses recorded under an old name show under
  the new one in Skill studio. `purlis persona lint` points out an old name in a persona's
  skills list or curation action. A command typed by an old name, such as
  `/purlis:purlis-compact`, no longer works: type the new one (#5724).

- **Get project's clone progress is the window's progress bar.** Git's stage names the bar and
  the percent is its value, and a screen reader hears the two once as they change, at most once a
  second, rather than twice.

- **Asks, Updates and Notices list their messages as rows, not boxed cards.** Each row has a
  round icon tinted by its state (waiting, finished, failed, a report), its chat or subject with
  its time at the right, what it is under that, its words, and its ways out as text links. The ×
  that ignores or dismisses a line takes the time's place when you point at the row or reach it
  with the keyboard. A long message from a chat shows three lines and "Show all · N lines". The
  words are the same as before (ADR 0119, #5649).

- Keep blocked on a browser login or a secret fill a chat suggests now holds for every chat of
  that persona until purlis restarts, and after two Keep blocked its chats stop asking. A new
  chat no longer starts asking again. A chat with no persona keeps its own count.

- **Keep blocked on a vault the chat's persona holds from the Allowed list links there**: the
  Notice that says Keep blocked does not stop the chat's runs, in the Inbox and on the refused
  vault's own Notice, has Open the Allowed list beside Dismiss, which opens Settings › Sandbox ›
  Network › Allowed. Where only the vault's tag holds it, there is no link. The sentence now
  names that whole path (#5939).

- **The hold on a workspace made during a sandboxed Claude Code chat is measured on Linux.** There, the record of the levels a chat started with is what keeps that workspace's skills from your chats until you accept it in the Skill studio (SK-41, #3097).

- **A project of a thousand workspaces scrolls as smoothly as a small one.** The title bar's
  workspace list, the switcher shown while the rail is hidden, and the command palette draw only
  the rows in sight. The arrows still reach every row, typing still finds any workspace, and a
  screen reader still says where a row stands in the whole list. The sidebar no longer sends the
  window every workspace's todo titles, only how many each has.

- **`memory_search` with no query shows the newest 20 memories, not all of them.** They come
  newest first, with a cursor to the 20 before them, so a chat never fills its context with a
  workspace's whole memory. A search with a query answers as before (#5500).

- A narrow pane head drops a chat's skill run and its browsers after the state's word, so a pane under 450px keeps the chat's name and mark (#5614).

- **New clones go to a workspace's `repos/` folder.** Every clone, from `purlis clone`, the
  window, `purlis init --clone-this-repo` or opening a repo as a new project, lands in
  `workspaces/<ws>/repos/<name>`, apart from the workspace's own notes and files. Clones already at
  a workspace's root stay where they are and keep working beside the new ones. `repos` is now a
  reserved name: a clone named `repos` is refused, and so is a new clone while an old clone named
  `repos` stands in the way, with how to rename it. The workspace layout is version 6, which
  `purlis workspace reinit` stamps without creating or moving anything, and a chat is told to keep
  its notes, scripts and artifacts out of `repos/` (ADR 0114, #5373).

- **Nothing pops up by itself any more.** A result about a workspace, or about the project or the
  app, no longer brings a card up over the window. It waits behind the dot on its workspace's chip,
  or on the status line's Notices item, with a "new" pip until you have seen it, and is listed in
  Notices as New. Pressing the dot still shows its card (ADR 0121 decisions 1, 3 and 10; #6004).

- **The status line no longer repeats counts the right bar shows.** Notices has no number there,
  and the Doctor item has none while Notices lists the doctor's findings (#6004).

- **The Notices dot is red or amber.** It is red while a Notice says something is broken, amber
  while one asks for a decision that stops no chat, and absent otherwise (#6004).

- **One turn says why it has nothing to show for a chat on an older runner.** Where a chat working
  on the branch runs on a runner too old to record its turns, a file's One turn diff names the
  chat and its runner or pool, where it said before that no chat had finished a turn (#5427).

- **The Notices centre's cards read like the Inbox's.** Each has a one-line head with its mark, its
  title, its state and age, Go to chat and a ×, with quieter words beneath. Where a notice has an
  answer, that answer is its one primary button, and everything else is a quiet link. A card waiting
  on your consent, a sandbox block among them, keeps its full waiting-colour edge, and other cards'
  outlines are softer (#5217).

- **Notices is a view of its own, first of the right side's Lower icons.** It opens in the right
  half of the Panel, so a Notice is read under Asks or Updates rather than in their place, and
  it lists the project's Notices, the window's own lines and Results such as a closed pane's
  Undo. Its warning sign shows a red dot only while a Notice says something needs fixing, never
  a number. The status line's Notices item and the palette's Show the Notices view open it.
  Every doctor finding stands together as one line, "Doctor: N findings", that opens to the list,
  with Fix all for the fixes one press may run, and Updates lists no Notices or doctor findings
  any more (ADR 0119, #5645).

- **A branch purlis already fetched from an older runner counts as brought back**, though it
  was squashed or rebased here since, deleted, or its workspace renamed. Such a runner no longer
  stays for ever for work that came here once.

- **One grip to move a pane or a chat.** Hover a persona's mark in the chats list or a pane's head
  and it turns into dots, with an open hand for the cursor; drag it, or press it to place with the
  keys. The head's grip button and the row's Place… button are gone, a view alone in its tab no
  longer has a grip over its content, and Place… in the menus is now Move… (#5290).

- A chat's mark is now the same on its tab, its row in the list, its pane's head and a folded pane's rail. One state wins when several are true: needs you, a failed task, working, waiting on tasks, idle, ended. A tab no longer shows a yellow dot for a chat the list shows waiting on its tasks.

- A task that failed shows a cross on its chat's mark until that chat has been on screen and focused, and the mark then goes back to the chat's own state. Its item in the Inbox goes at the same moment.

- **A run asks once for everything it needs.** A chat's `secret exec` that lacks keys, a
  program's Allow or a vault's raises one notice with all of them: a masked field for each
  missing key, and the program's Allow, or, for a declared tool on a vault its persona can't use,
  the vault's own Allow. What you type is stored even on Don't allow, and the chat hears "filled,
  but the program was not allowed". Allow once is used by the run it was for, so a run that had
  to wait doesn't ask again on retry (ADR 0112, #5242).

- **A secret's notice in the Inbox is one card that says who says what.** A Secret ask and a
  run's notice show purlis's facts (the program's path, what it is handed, the hosts this chat
  can reach, the vault, why there is no Allow) in a panel of their own, and the chat's command and
  purpose apart, quoted, with every hidden character shown by name. Each missing key gets a box
  with a visible edge, or an entry you keep, picked by vault; the Allow is one choice, narrowest
  first, saying what it does, and Always allow shows its line, and whether other changes still
  wait, before you press it. Remember this need works on a run's notice too.

- **One notification per chat.** A chat that waits on you has one notification, naming the
  chat, its project and what kind of ask it is. Its next ask updates it, and it goes once you
  have answered everything the chat asked (on Linux; macOS and Windows keep a notification until
  you clear it, so there a chat is not told twice). When three chats or more ask within a few
  seconds of each other, in any projects, one notification counts them all: "3 chats in 2
  projects need you". A click brings the chat forward with its ask open in Asks. Only what the
  ✋ counts notifies, so a snoozed chat does not (#6003, settles #2108).

- **A large project opens and idles without the file watcher's cost.** On macOS purlis watches
  the whole project with one stream, so setting up the watch no longer grows with the square of
  the folder count. An open window no longer re-reads every watched folder every five seconds.
  A full catch-up runs only when the system says it dropped events. On Linux, a folder the
  system will not watch, once it is out of watches, is checked on its own every few seconds,
  and the sidebar is no longer read from disk on every request (#5499).

- **Open in terminal on a pool's profile now types `purlis profile sign-in <name> --pool <pool>
  --in-terminal`.** It checks the pool, as the window does, every time it runs, including when you
  run it again from your shell's history. Then it runs the runner's own sign-in in that tab for
  you to drive. The line the window just typed goes straight on. Run again later, it asks you in
  the purlis window first.

- **opencode's own sign-in on a runner is carried to the runner that replaces it,** as Claude
  Code's and Codex's are, when the runner it came from kept its opencode chats out of that
  folder. Before, opencode signed in again on the new runner.

- **Opening a task beside names the chat it opens beside.** The button on a task's line in a
  chat's tasks menu says "Open talk beside steward 1", as the palette and the task's menus
  already did, not "beside its session". The palette's row for the task in front says "Open
  the task in front beside the chat that asked for it" (#5679).

- **A skill that runs its own scripts can pass its outcome evals.** Where a skill's
  `allowed-tools` names Bash, each eval's chat in a grade has a shell, in every arm. The shell
  writes only in the chat's own folder and a temp folder of its own, and reaches no network. The plan says which evals use one
  before anything runs, and a grade's token and dollar limits still hold (#4476).

- **The pane foot's skills warning says what a press does in its row's words**, as the foot's
  other parts do (`Skills of …`). Where nothing can be pressed it says only the warning (#5366).

- **The theme check holds the pane foot's warning number on the terminal.** A low cache's figures
  are drawn in the waiting or the failed colour on the terminal's own background, and the check
  now holds both at 4.5:1 there. A theme that falls short is drawn as you wrote it, terminal and
  colours alike, and the check names the pair to fix. A theme's colour for the foot's numbers no
  longer covers a warning's (#5262).

- A pane's Split right, Split down and Close are always visible in its head, not only under the pointer. A tab with one pane shows no Close, in its head or the palette, because the tab's own × does the same. A view alone in its tab has no controls over its content: its splits are in the palette and on its tab's menu (#5318).

- A chat pane's notices are compact rows under its head: one line each, with its icon, who asks and what about, its age where known, its own buttons and a × that hides it from this pane. An unanswered ask stays in the Inbox and on ✋. The band folds to one line naming how many wait and on what, and an opened message shows whole, the terminal giving way down to three lines.

- **A project's slow panels load on their own.** While a project's save status, stale repo check
  or alerts are still being read, the title bar and the Inbox each say so with their own mark, and
  the chats are usable meanwhile. The alerts of every project are read once when a launch's restore
  settles, rather than again for each project that joins (#5325).

- **Panes make room while you drag.** The other panes move to where they'd end up, and what you're moving shows in its place, so you see the result before you let go. Each terminal keeps its size until the drop and shows the size it will take. Esc, letting go outside any place, and the window losing focus put everything back as it was (#5291).

- **A theme that fills a panel, a popover or a dialog is held to everything drawn on it.** The
  parts check measured only buttons and menus on a fill a theme gives a surface. It now measures
  every family the window draws there (a field and its help, a list's rows, a badge, a meter), so a
  fill their words vanish on draws the surface's default instead, and the check says which family
  broke there (#3268).

- **A persona's own sandbox hosts are written one per line, sorted.** The Settings form writes
  `[sandbox.personas.<name>] hosts` as the project's hosts are written, one host per line in
  order and once each, so two machines that each add a persona host merge without a merge
  commit (#5853).

- **The project's settings file lists its allowed hosts and browser sites one per line, in
  order,** so two machines that each add one change different lines (#5824).

- **The save badge says when the project is stale.** While a save's request is open and the
  remote's branch could not be fetched, the badge says *stale*, and its tooltip and the Saving tab
  say since when and why. The next look tries again (#5824).

- **A project's search index builds about six times faster.** The first index of a project at
  the scale spec's size (1,000 workspaces, 200,000 memories) took about two minutes; it now
  takes under 20 seconds, because each memory is read and scanned for secrets on every core while
  the index is written. What the index holds, and every answer it gives, is unchanged.

- The build provenance of a dev build, of the runner image and of the cluster operator image
  now names the commit each was built from. Dev builds and the runner image are started by the
  push to `main` and built once that commit's checks pass, and the operator image is built by
  the operator's own checks of that push, so `gh attestation verify --source-digest <commit>`
  checks the commit. Before, a merge that landed while the checks ran made the provenance name
  that later commit.

- **`purlis signing` no longer needs GitHub's `gh`.** `setup`, `status` and `check` ask the
  running purlis window, which asks GitHub with its own sign-in. In the terminal you add the key
  on GitHub's page by hand, and the window checks that GitHub has it. With no window running,
  `setup` gives the steps and says to open purlis. `setup --by-hand` is gone, since adding by
  hand is now the only way from the terminal (#5625).

- **Quitting and Restart to update name the work of its own a chat would lose.** A chat whose
  turn ended while background shells, monitors, helpers, workflows or a wake-up of its own were
  still in flight is listed in the quit warning and in Restart to update's question, which say
  what will end, as "1 monitor and 2 background shells will end". Both still go ahead when you
  choose (#5748).

- **The project rail and the activity bars wrap at their ends**, as the tab strip does: Down on
  the last project or view goes to the first, and Up on the first to the last. Both are now drawn
  on the window's one tabs family, so their keyboard is the tab strip's (#3134).

- **Rename says what it renames.** A tab's Rename is a dialog that says what it changes: in a
  tab with one chat it renames the chat, so the tab, the list and the pane's head change
  together, and a tick for *Only this tab* gives the tab a name of its own instead. In a split
  tab it names the tab only, and says the chats keep their names. A tab with a name of its own
  can go back to its chat's with *Use the chat name again*. A view's tab has no rename. Rename
  opens on a double-click, on F2 and from the tab's menu, and replaces the box that opened on
  the tab itself, so leaving it no longer saves the name: Enter does (#5343).

- **Rename chat… is in every chat's menu**, by that name: a pane's ⋯, the list row's menu and
  the palette, and F2 on a chat's row in the list opens it. It opens on
  that chat's own name (#5343).

- **The same failure of purlis's own is one report draft, whichever chats meet it.** A failure
  with the same command or tool and the same kind of error, in the same purlis version, is
  counted on the draft already waiting, or on the report once it was sent, instead of drafting
  it again. A report a chat writes now names its harness, the harness's version and the kind of
  persona it ran as, never the persona's name (#5683, #5697).

- **"See exactly what's sent" shows what purlis's own report builders make** of a sample crash
  and a sample draft, byte for byte, instead of a description of them (#5688).

- **A report draft's Always send is off where your project turns sending without asking off**,
  and says so, instead of offering a press the project refuses (#5696).

- **Words an agent adds after Always send sent its draft are sent after it**, linked to it, and
  listed in Settings › You › Reports, instead of being lost (#5695, #5697).

- **A report waiting offline counts against the day's five unread drafts on the day it leaves**
  (#5693).

- **A Crash report, and a failure of purlis's own that it sent to the Report intake, carry their
  fingerprint as a tag too.** The steward's daily routine reads it back from Sentry and adds a
  repeat of a filed bug to its issue instead of filing it again (#5701).

- **Resolve settles a save's conflict in a folder beside the project.** The project itself is
  never left mid-merge: chats go on working in it, a quit leaves it as it was, and the resolve is
  there again when purlis starts. Continue moves the project in one step, and says which file is
  in the way if a chat changed one meanwhile. Abort removes the folder and changes nothing
  (#5824).

- **A setting both machines changed is chosen in the window.** Where both machines changed the
  same key of `charter.toml`, Resolve shows the key with both values and you choose one. No
  conflict marker is ever written into the file (#5852).

- **Saving after the remote moved replays this machine's commits without a rebase.** The
  project moves once the whole line is made, so nothing is left part-way (#5855).

- **Resting on a tab while you drag brings it forward after a moment, and Esc takes you back.** The tab shows a mark while it's about to open, and nothing changes until you let go (#5294).

- **Placing a pane with the keys shows the same preview as dragging.** As the arrows move between places, the panes make room, with their sizes and what the drop would do. Esc puts the panes, the front tab and the keyboard back as they were (#5293).

- An update of one skill that fails part way now puts the skill's files back as they were, instead of leaving what it wrote.

- **A run's notice offers Allow for an ssh jump to a scoped link-local host** written as ssh
  reads it in a jump, `-J u@[fe80::1%%en0]`, as the last hop. Any other `%` in a jump still gets
  no Allow, since ssh expands its `%` tokens in the command its shell runs (#5893).

- **A chat on a pool's runner is held to where it runs.** A message or report between a runner
  chat and a task on this machine says which pool's runner it came from. A chat with tasks still
  working on this machine stays here until they end or report. A launch no longer starts a task a
  runner chat asked for: it waits under that chat for Try to start again or End task. The chats of
  one pool hold at most 8 opens and dispatches for the person together, and a runner chat's git
  work is done on its runner, never on this machine (ADR 0078).

- **A runner's own policy decides whether a chat it runs may ask about a host.** Where a runner's
  administrator policy turns live asks off, or keeps no Allow for one chat, a chat the runner
  composes or the desktop relays to it refuses an unlisted host and asks the window nothing, as it
  already did for the runner's services.

- **A Docker pool's runner is checked against every container setting that could give it more.**
  Its container is refused when it carries extra groups, a device cgroup rule, protection from the
  out-of-memory killer, a cgroup of its own choosing, a `/dev/shm` its pool did not give it,
  unmasked kernel paths, or any setting the check does not know.

- **A cluster pool's credential plugins are kept by their full path.** The pool's own copy of the
  context names each plugin by the path its add found and held, so no later kubectl run looks it up
  on its PATH. A plugin inside a Python virtual environment is held with the whole environment, and
  one whose package is inside a chat's project is held with its package.

- **The cluster operator holds more of what it makes.** Only the operator makes a purlis pool's
  SandboxTemplate or changes its spec; each runner namespace lets its pods out only where a network
  policy lets them (`purlis-deny-egress`); what the operator makes it holds as applied from the
  first, so a field it stops setting is taken out; and a policy binding that gives the policy
  parameters does not put it in force.

- **Saving's "Sign commits" switch is now "Sign saves with your own git setup"**, so it reads
  apart from Signed commits, which signs the commits the app makes for chats (#5626).

- Browsers: a chat can't close a browser while the person has it taken over. The answer says why, and the person can still close it themselves.

- **Settings' groups and its Edit as TOML files are one page navigation**, the one the Skill
  studio uses: the group shown is marked by the accent and a bar at its start, and the files at
  its foot are one Tab stop, walked with the arrow keys. A tab's close says its note on a second
  line of its tooltip (#2930).

- **Seven more built-in skills say when they are for, and which skill to use instead.** The
  descriptions of `purlis-browser`, `purlis-theme`, `purlis-upgrade`, `purlis-tidy-memory`,
  `purlis-repo-work`, `purlis-curation-action` and `purlis-skill-studio` now open with the
  situation in the words people use and end with a "Not for … — that is …" line for each skill
  they are confused with, for example a theme for how the purlis window looks and repo work for
  the styles of your own app. Each now has trigger tests, at the standard bar, and every built-in
  skill purlis ships must carry them.

- **A task you asked for notifies you when it fails, or when it finishes after a long while
  with you away.** A failure names the task, its project and the first line of what went wrong.
  A finish notifies only after more than 5 minutes of work while no purlis window is in front;
  the minutes are `notifications.finishedAfter` in the layout file. Nothing else in Updates
  notifies, and the bell counts as before. A task one of your chats dispatched reports to that
  chat and notifies you of nothing. While Quiet these wait, and the one notification afterwards
  counts them ("While you were quiet: 2 finished, 1 failed"). A click opens the task's line in
  Updates, its report open (#6005).

- **The palette's row for a chat's skills says what it does: Show skills of …** (it said
  *Skills of …*), and the pane foot's skills warning says the same (#5543).

- **Signed commits says what removing another device's key does, and asks first.** The list is
  now headed *Signing keys of your other devices*, with a line saying that removing one stops that
  device's commits showing as Verified until it sets up signing again. Each key shows its device's
  name, the day it was added, and *Older setup* when an older purlis titled it. **Remove…** is a
  quiet button that opens a confirmation naming the device. The red button is only in that
  confirmation, and it says this Mac is not affected.

- **A chat's unsigned commit gets one quiet line**, "Not signed: signing isn't set up on this
  Mac", pointing at the Signed commits card, in place of a warning (#5630).

- **Change a chat's skill budget where you see it, for each program.** The Skill studio's Routing
  page has a row for Claude Code, Codex and opencode, each with **Change…**, and the numbers on the
  Skills view and a chat's pane link there. As you type, it says which skills new chats would list
  or hide, about how many tokens that costs per chat, and whether it passes what the program can
  hold. Each number says where it comes from, with **Reset to default**. New chats get the change;
  open chats keep theirs and offer a restart (#5353).

- **The skill editor asks lint extensions as an edit opens, not as you type.** Each extension
  that declares `skill-lint` starts its program confined for every question, which took 345 to
  449 ms on macOS, longer than the 200 ms pause after a keystroke the live lint waits. purlis's
  own lints still check the text as you type, and what an extension said of the text as it
  opened stays shown beside them. Once the skill is saved, the Skill studio's list asks the
  extensions of the saved skill, within its own time, as it does for every skill (#3093).

- **The built-in skills and a chat's briefing say each rule one way.** A website login is the
  `browser_login` tool; a chat commits in a branch folder with `purlis worktree commit`, in a
  terminal too; a helper does read-only work; a chat does what it can itself and dispatches when
  the work needs another persona; a sandboxed chat's change is saved by the app and a terminal
  chat runs `purlis save`; `--force` only on your word; a chat never asks for a secret's value;
  it asks you through `ask_operator`; and a task writes its session record before it reports.
  A check fails the build when a shipped skill, the briefing or a purlis tool says the old side
  again (#5722).

- **A chat whose profile's key is missing from its vault asks for it in the Start dialog.**
  The dialog shows the Secret ask for that key. Fill stores it, and the chat starts without a
  second press of Start. Not now starts nothing and says so. It is the same ask as the one in
  the Inbox, so answering it in either place closes it in both. Before, the start was refused
  and you filled the key in the Inbox, then started the chat again.

- **The status bar is the chrome's height, and its project copies its directory.** A status bar
  item's keyboard ring is drawn inside it, as a tab's is, so the bar no longer keeps empty room
  above and below its items. The project's name is a button that copies its directory, and says
  so to a screen reader (#1922).

- A finished chat reads Idle only while its folder holds uncommitted work; with nothing left behind it reads Done (#6014).

- **A chat the Skill studio started stays on its skill's Runs once it closes, with when it
  finished.** Its row offers Review draft while the draft it wrote waits on Drafts. The Grades
  card's running job says how long it has run and has Stop it, and a project's own skill names
  the project on its buttons: "Ask a chat at purlis-demo to write these" (#5248).

- **The app's program is now `purlis-app`, not `charter-app`.** On Linux the `.deb` installs
  `/usr/bin/purlis-app`, and its menu entry starts it under that name. A script or a shortcut of
  your own that runs `charter-app` needs the new name. No `charter-app` command is kept. The
  `purlis` and `charter` commands are unchanged. On macOS the program inside `purlis.app` is
  renamed and an update replaces the whole app, so nothing changes for you.

- **The app runs git only where the repository's settings, hooks and index have one name.** A
  save, update, push or resolve of the project, and a fetch or an update of a clone, stops,
  naming the file, where `config`, `HEAD`, `index`, a hook, a file under `info/` or
  `objects/info/alternates` has a second name (a hard link). A hook or a setting that is a
  link is left to git as before. Where the refs or the reflogs are a link to another
  repository's folder, as `git-new-workdir` makes, the save says so and that
  `git worktree add` shares a repository without one.

- **A push the app makes for a chat is never signed**, whatever `push.gpgSign` says: a chat's
  save of the project and its `purlis worktree push`.

- **The ✋ counts what waits on you in every open project**, not only the one in front, and says
  how many of them are elsewhere ("3 things wait on you, +1 elsewhere"). The Dock badge shows the
  same total on macOS, and on Linux where the desktop shows one, and nothing when nothing waits.
  Asks opens on All projects; This project and This workspace narrow the list, never the ✋. A
  snoozed chat is not counted.

- **⌘T opens an empty tab and ⌘N starts a new chat** (Ctrl+T and Ctrl+N off a Mac, where a
  chat's terminal keeps them for its shell). ⌘⇧T is still New shell. Until now neither key did
  anything in purlis, and `+` was the only way to an empty tab (#5292).

- **An empty pane offers New shell beside New chat**, and says you can drag a chat in from the
  sidebar. A tab with nothing in it is called Empty tab until it holds something (#5292).

- **The project index builds on about half the CPU.** Reading a memory for secrets before it is
  indexed asked each rule once instead of twice, and read personal data a line at a time instead
  of across the whole memory, where one character that is not ASCII sent those rules to their
  slowest search. A new file is read with one lookup of its path instead of two, and files are read
  on two threads for each core, which takes up the time a read waits on macOS. What is masked is
  unchanged, span for span (#5672).

- **A chat leaves the project root's git to the app.** A Claude Code chat no longer writes the
  project root's `.git`, wherever in the project it stands: `git add`, `commit`, `fetch`, `stash`
  and `rebase` there stop, and read-only git works as before. `purlis save` in a chat the app
  started is run by the app, with the same `-m` and `--no-push`, and its commit names the chat.
  It is never signed: `--sign` and `[plane] sign` are not taken for a chat, and the save says
  so. You sign a save by saving from the window or in your own terminal. That save leaves out files the chat cannot read, the commit's trailers are the
  app's, and it brings in nothing from the remote: `--pull` is refused, a remote that moved
  leaves the commit unpushed until `purlis worktree update`, and a project that saves through a
  pull request is committed and pushed by your own save. Codex, opencode and Claude Code on a runner save the project with
  their own git, as before. A skill change or a workspace rename a chat makes is saved by the
  next save. Your own terminal works as before.

- **The app runs git only in a git folder whose files are its own.** A fetch or an update of a
  clone stops, naming the file, where `packed-refs` or a file under `logs/` is a link, has a
  second name or cannot be read. A save, update, push or resolve of the project stops the same
  way for those and for the files git keeps beside them, such as `COMMIT_EDITMSG`, `ORIG_HEAD`
  and the merge and rebase state. The app's fetches run no automatic maintenance.

- **A chat with no workspace yet is told only the ten most recently used.** Where a project has
  more than ten workspaces, the line asking a new chat to confirm one names the ten used most
  recently, then "…and N more: `purlis workspace list`", in place of every name, which on a
  project of a thousand workspaces was about 16 KB in every chat (#5486).

- **Settings › Theme's gallery checks only the themes that changed when it is read again.** A
  choice in the gallery reads it again; the built-in themes and every folder that draws as it did
  are no longer put through the theme check each time, and Duplicate as my theme no longer checks
  every theme in the gallery to find its card (#3597).

- **Importing a theme takes a fraction of the time it did, a light one most of all.** Tuning
  an import's colours no longer measures every workspace tint for a step that cannot win, and
  holding the consent floors measures the tints only once the theme as written clears them
  (#3597).

- **Updates is two lists: Mine and Project.** Mine (from your chats) lists what your own chats
  and tasks did, and the bell counts its unread lines in the plain colours. Project (from the
  project) lists the doctor's findings and what teammates changed in the project: its hosts, its
  Internet access presets and its dispatch grants, which are no longer among the Notices and are
  never counted. Updates opens on Mine, narrows to this workspace as Asks does, and Mark all read
  and Dismiss all act on the list shown (ADR 0119, #5646).

- **A chat can ask for a key and its program's Allow in one notice.** `secret_ask` and
  `purlis secret ask` take `--program`: for curl you can fill the key and Allow for this chat
  together; for any other program the run asks on the exact command. Chats are now told to prefer
  simply running the command, and a "filled" answer says whether the program was allowed (ADR
  0112, #5244).

- **A chat's skill run and its browsers are on its pane head, not its tab.** A tab says which
  chat it is and whether it needs you; what the chat uses now is beside its state on its pane's
  head, and each browser is still a row under its chat (#5524).

- **A head with no room for its state's word shows its tasks' button as a list,** not a second
  copy of the state's glyph beside the chat's mark (#5524).

- **A split tab's picture draws a tile for each pane,** so three panes no longer look like four,
  and past four it is a 3 × 3 grid (#5577).

- **A view beside a chat is named by its own title,** such as its file's name, in its tab's hover,
  and the tab takes that title when the view becomes its first pane (#5577).

- **Where a chat runs is never guessed.** While purlis can't read its machine store
  (`machine.json` in its config folder), a chat started in a workspace is refused in words that
  name the file and how to fix it, rather than started on this machine. A store that reads but
  doesn't parse is kept beside it as `machine.json.unread` before it is replaced, and a project
  whose chats run on a pool is no longer forgotten when you open more than 64 others.

## Deprecated

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

## Fixed

- **A row in a pane's band keeps its room inside its edge.** Its icon, words and buttons no longer
  touch the row's border, and in a narrow pane its buttons are no longer drawn on its bottom edge,
  looking cut (#5443).

- **A sandbox block's Notice shows at once.** The app tells the window of a block before it
  writes the block to this machine's network record, and that record is written under a lock in
  purlis's own folder, which no chat can hold, so a chat holding the record's folder no longer
  delays a Notice by ten seconds or keeps the block out of Blocked lately.

- **A text box, a select and a combobox have a visible edge in Purlis**, so an empty box can be
  found on the window and in a panel or dialog. The edge holds 3:1 against what the box stands on
  (WCAG 2.2, 1.4.11). Themes can set it with the new `border.input` token; a theme that leaves it
  out draws its boxes as before.

- A browser login notice goes as soon as the chat's call is cancelled, instead of staying for 15
  minutes and blocking the chat's next try.

- A secret fill's notice goes as soon as the chat's call is cancelled, as a login's does, instead
  of staying until you answer it or 15 minutes pass.

- A secret fill waiting for you to add its key stops waiting as soon as the chat's call is
  cancelled, and its Secret ask goes from the window unless another chat still needs that key.

- **A chat's card goes when the keyboard leaves its row.** If the card came up at the moment the
  keyboard moved off the row, it stayed up beside the Chats list with nothing on the row.

- **A chat shown only in a tab window says its notes once.** They stand on its pane's band in that
  window, and no longer in the project window's Notices centre too. Clearing a note clears it in
  every window of the project (#5225).

- **A chat's pane no longer freezes half-drawn after the waiting strip comes and goes.** A
  program on the alternate screen, in a pane made shorter and taller again, could make the
  terminal throw while drawing, after which it drew nothing more and took no keys until the chat
  fell out of the last three shown. The terminal now keeps the alternate screen to its rows, and
  a pane whose terminal stops drawing what it is sent starts its terminal again on its own
  (#5478).

- A background shell or monitor a Claude Code chat started now ends with the chat's harness when the harness exits on its own or is ended from outside purlis, not only when purlis ends it. Before, such a shell kept running on its own, with nothing in purlis that could still see it (#5806).

- **A chat's browser asks you on the chat's own pane again.** What its browser may reach, a site
  new to the project it was sent to, and a page that waits for you (a consent page, a sign-in)
  are each drawn on the chat's pane with their answers, Take over among them, as well as in Asks.
  Before, a person looking at the chat saw nothing to press, and Asks offered no Take over. A
  secret a chat asks purlis to fill is drawn on its pane too, as it was meant to be.

- **A chat with no row yet is called its own name, never its tab's.** A chat started into a split
  is called what its row will say from the moment its pane opens, and a tab left with only that
  chat takes that name, never its session number (#5308).

- **What a sandboxed chat makes in its first moments is told.** Where a chat's folders took longer
  to record than its start waits, a name the chat wrote before the record reached it was taken
  as there from the start, and the Inbox never said it had appeared. Now anything made after the
  start began is told, and a look at the end of a turn waits for the chat's record (#5555).

- From a chat, `purlis skill edit <name> --from <file>` and `purlis skill new` now leave a draft in
  the Skill studio's Drafts, where you review its diff and accept or decline it. Before, they were
  refused, and their message promised a draft that was never made. Nothing is written to the skill
  until you accept.

- **A chat that opens while you drag a pane no longer takes the keyboard.** The keyboard stays where it was until you let go, so nothing you type goes into a tab that's only shown as a preview (#5523).

- **A secret fill you take the browser over from keeps its mark.** When you take a chat's browser
  over just as purlis fills a secret, and the value is already in the field, the browser's view now
  marks the field `secret: <key>`, and the chat is told the value went in. Before, the chat was told
  the fill was interrupted and the field had no mark.

- **A browser you watch is the size its view's menu shows from its first frame.** Before you
  picked a size, its page was painted at whatever the hidden window gave it, shorter than the
  Laptop size the menu said (1280 by 713 on one Mac, 1280 by 600 on another). Each page now
  starts at the size the menu shows.

- **A shell or a Skill studio chat started in a tab window for another workspace opens in the project window as what it is.** A shell wears a terminal's mark, and a studio chat has the chip that says what the studio started it for (#5573).

- **A manual-only skill that narrows the chat now loads when you name it in a Claude Code
  chat.** The chat used to be told to start it as `skills:<name>`, which Claude Code refuses for
  a manual-only skill, so it stopped there. purlis now loads it and says its narrowing is not
  applied. To run it with its narrowing, type its slash command yourself, such as
  `/skills:<name>` (#5880).

- **A link on the launcher stays readable.** With no project open, the welcome's links (a door
  page's back, Open a folder) stand on the window's deepest surface, and the theme check now
  measures a link there too. An accent or a theme whose link fell under the text floor on that
  ground alone, such as a violet over Purlis Classic light, keeps a link colour that reads, and
  the check says so (#5347).

- **A browser login refused before it ran no longer spends its Allow once.** Where the browser had
  ended, or held another login's site, or was held by a page holding a filled secret, the chat's
  Allow once stays its own, and the browser's record of its session is kept, so the next call can
  still answer "already logged in" without reading the vault again (#5251).

- **A view alone in its tab uses its whole first row.** Its row no longer ends in an empty gap
  kept for split and close controls it does not draw (#5442).

- **A pool runner made again keeps where its skill runs were logged up to.** Past 256 kept
  cursors, the window used to let go of the lowest, so the cursors of a runner made again (whose
  log counts from the start) went before those of the runners it replaced, and the skill runs its
  chats made while no window was open never reached the studio. The runner kept or held least
  lately now goes first (#5657).

- **A workspace you make is drawn pinned at once.** Making one from the title bar could draw it
  unpinned, or not at all, for a moment before its pin landed (#1926).

- **Several chats remembering at once are no longer refused as busy for nothing.** A write to a
  memory store waited for its lock by asking again every 20 ms, so a writer that let go and asked
  again at once could keep taking it, and the other was refused after 5 seconds. A writer now
  waits in turn and gets the lock when it is let go (#5559).

- **A merge of a merge pushes without being taken for an allowlist change.** The pre-push scan
  compared a merge's allowlist with the parents history simplification left it, so a merge one
  of whose sides never touched the allowlist was refused as writing its own; it now compares
  against the merge's real parents (#6035).

- **Moving a chat to a pool with git older than 2.40 says which git and what to do.** The move
  used to stop on git's own usage text ("unknown option: --attr-source=…"). Now it names the
  git that ran and its version, says a move needs git 2.40 or later, and leaves the chat where
  it was. This applies on your machine and on the runner.

- **A persona's `workspaces/<ws>:<name>` skill that is not there is said on the chat's tab.** The
  Notice names each one as the list writes it. A workspace the project does not have is said
  apart from a skill its workspace does not hold, each with its fix, and Show in Skills leads to
  the Skill studio on the persona (SK-5b, #6040).

- **A skill the router hid now loads on Claude Code when it narrows the chat.** Before, the chat
  was told to start it with Claude Code's own Skill tool. That tool didn't know the skill, so it
  answered "Unknown skill" and the skill never ran. A skill installed after the chat started had
  the same problem. Now the skill loads, and the chat is told its narrowing isn't applied. A skill
  the router kept still starts through Claude Code, with its narrowing.

- **A pane's band goes in one step when its last warning does.** The terminal is fitted once,
  to its full room, and the chat's harness is told one size and redraws once, where before it
  was fitted a row short for a frame first. A band raised again starts at its top, however far
  it was scrolled when it went (#3437).

- **A chat's pane that froze while you typed comes back on its own** (#5478). When a pane has
  been typed into and its program has sent nothing back for 10 seconds, the pane starts its
  terminal and its view of the chat again and is sent the screen as it is now. Before, switching
  tabs left it frozen, and only showing three other chats brought it back. A chat starts again
  at most once every five minutes. The cause is still being looked for.

- **A pane's trouble notice draws as trouble.** Where a pane draws a notice as a box, such as
  "MCP server figma did not start", it is on the danger wash with the octagon, no longer on the
  warning wash with the triangle. A consent stays on the warning wash with its lock, whatever
  its tone (#4875).

- **A save request that conflicts with its target now recovers by itself** (#5338). In the
  `pr` and `pr-merge` modes, a save and the app's own look bring a target that moved into the
  plane as one merge commit where the request needs it, and the request is pushed so it can
  merge. A request that merges as it is gets no merge commit. Session indexes, the
  todos index and the `## Sessions` line are rebuilt from the records and todos, so nothing is
  lost and the count stays true. A conflict in something a person wrote stops the save and names
  the file with nothing changed, and Resolve in the Saving view settles it in one stop. Saved
  planes pick up the new merge rules with their next save.

- **A save in `push` mode no longer stops on a memory index both machines added to.** Its
  replay rebuilds a memory store's `MEMORY.md`, a `sessions/index.md` and a workspace's
  `todos/MEMORY.md` from their sources, as every merge purlis makes already did, and merges the
  settings file key by key, so only what a person wrote can stop it. A new project's
  `.gitattributes` no longer carries the two memory `merge=union` rules; an existing project
  keeps them until `reinit` (#5864).

- **A run on a vault its persona may not use no longer offers that vault's own entries, unless
  its notice offers the vault's own Allow.** The run was refused after the person's Allow. Type
  a value there instead (#5744).

- **A browser site you remove is asked about again.** A page holding a secret purlis filled no
  longer leaves for a site you removed, for one chat or for everyone, by its server's redirect
  without asking, because you had allowed that site when asked before.

- **A browser reopened from kept logins reaches its sign-in host from the start.** For a login
  whose form is on another host, the reopened browser reaches that host as one logged in afresh
  does, so the site's own redirects, hidden frames and token refreshes work before the chat calls
  `browser_login`, and the chat stays kept off that host's pages outside a sign-in (#5167).

- **A bug report masks the name of a clone in a workspace's `repos/` folder**, as it already did
  for a clone at the workspace's root. A chat in a branch folder of such a clone also gets that
  branch's repo skills.

- **A move that does not finish no longer hides a change.** Where moving a chat's work did not
  finish and purlis put the clone's index back, a file changed at the same size in the moment the
  index was last written could read as unchanged on Linux, missing from status and diff. The index
  put back now keeps the time it had (#5548).

- **A menu row and a list's option are named by their words alone, and say the rest once.** A
  screen reader hears a menu row's key as its keyboard shortcut and the line under its words as
  its description, where some rows used to leave that line out, and a theme card in Settings ›
  Theme is named by its theme with where it comes from as its description. Voice control finds
  each by the words you see (WCAG 2.5.3).

- **A run of another workspace's skill a persona names is that workspace's copy's.** Its record
  says whose copy it ran, and what it ran is read from that copy, never from a copy of the same
  name in the chat's own workspace. The skill's versions count it, and `skill_runs` tells it
  apart. With the Skill studio flag on, the studio's row of the copy edits, shares and shows the
  history of the copy in its own workspace (SK-5c, #6041).

- **A runner chat's skill runs reach the Skill studio after the window was closed.** The window
  keeps, for each pool runner and project, how far it logged the runner's skill runs, and reads
  on from there when it links again, so runs a runner chat made while no window was open are
  logged once each. A run's version is the one the runner ran, not the one on this machine by
  then. Behind the Skill studio's flag (#4219).

- **A runner goes only with its work fetched.** A draining runner stays until every project that ran chats on it has fetched from it, and a project whose window is closed fetches once it opens. If the fetch can't finish the runner stays, and the log says which project it waits on.

- **A fresh runner's check no longer holds up other chats' branches coming back.** The check runs outside the one-at-a-time fetch, the background sweep asks a runner whose check took minutes far less often, and Fetch again still asks at once. A check that took long looks again before the runner goes. Opening a task on a very large kept runner now says what to do if its check is too slow, and a failed release after a Discard no longer says "Its work is all here" (#5393).

- **On a pool's runner, a persona's tool nobody allowed says the Allow is wanted too.** Where its
  program is not in the image, its sentence also says it runs only once the persona's hosts and
  tools are allowed on the desktop that starts the chat. A program of that name installed after
  the chat started now runs, instead of the refusal hiding it. A chat with nothing allowed finds
  only its own refusals first on its `PATH`, never another chat's tool shims.

- **The Activity tab decides which reports are purlis's from the task's record, never from its
  words.** A task whose report repeats one of purlis's sentences is drawn as the task's, and a
  task purlis or you ended keeps its ending when a later report arrives.

- **A task renamed with a backtick or one of purlis's marks keeps its report's heading whole.**
  Its name is written with those marks replaced, as purlis's word that it was stopped writes it.

- **A tab window keeps its strip when you press a chat from another workspace.** The chat opens in
  the project window, which comes forward, instead of in a new tab of the tab window (#5330).

- **A tab window keeps its strip when a chat from another workspace starts or comes back there.** A Fix, a theme or skill chat, or a resumed chat opens in the project window instead. So does a press made just as a tab window opens (#5533).

- **A theme's colours for a tab's × no longer cover its look under the pointer.** A theme
  that set `tabs.closer.text` or `tabs.closer.fill` drew that colour on the × even under the
  pointer, over the quiet hover, where the theme check never read it. The × now keeps its hover
  there, and the check reads its words on it over the tab's own fill, so a theme whose tab fill
  hides them draws that tab's default (#3269).

- **A task's report reaches a busy app.** `purlis dispatch report`, `note` and `ask` used to
  give up after two seconds. When the window was busy, for example after an update had brought
  every chat back at once, a task was told no app was there, and its report went unsent. These
  commands now wait up to twenty seconds for an app that is there. Where the app still does not
  answer, the task is told so and to send its report again (#5845).

- **A tab's tasks menu no longer opens under a pointer that has not moved.** A tasks button
  drawn under a parked pointer could open its menu from the first move it heard (#3521).

- **A vault grant on this machine is no longer dropped by an unrelated write.** When another
  field of this machine's sandbox file was off its shape, the next grant, revoke or chat count
  rewrote the file from defaults and took away the vault grants the window still showed. The vault
  grants are now kept when the file starts again.

- **Allow stays told apart from the suggested answer whatever state either is in.** The theme
  check and the window held Allow apart from the primary button only with both in the same state;
  a theme could draw Allow under the pointer just as the primary at rest. Every look of Allow and
  of danger is now measured against every look of the primary, and a theme that brings a pair
  together draws purlis's own look there. An imported theme is tuned to keep them apart in every
  workspace colour too, so an imported scheme such as Dune's light one may come out with a
  quieter accent than before (#2994).

- **The Allowed list drops an Allow once as soon as it goes.** It used to keep the row until it
  next read the list when the Allow once was used by its run, dropped because a later check
  refused that run, or went with its chat when the chat closed (#5800).

- **An agent identity's vault stays registered until the identity is removed.** `purlis vault
  remove` now refuses a keyring vault that holds an agent identity's key, as `vault destroy`
  already did, and the refusal of a GitHub App's code exchange says to delete that App and close
  the browser tab. Finish allows exactly the lines it wrote to the project.

- **A browser's start no longer fails on one bad answer from a server.** When the browser
  engine's manifest or archive answers with a server error, such as a 504 from GitHub, purlis
  asks again twice, a second and then two seconds later, before it gives up. Until now one such
  answer could refuse an outdated engine that a newer one would have replaced.

- **An older runner that was stopped, or whose container was removed outside purlis, is no
  longer removed with its home unchecked.** purlis can't ask a stopped runner what it holds
  without starting it, so the runner stays. Settings › Pools and Devices say it was not checked.
  Start its container and purlis checks it at its next look, or let it go in Devices.

- **A screen reader hears every line a list's option draws about it.** A skill's description in
  Use a skill, a memory's search snippet in the archive, a search hit's line number, and where a
  refused theme comes from are now read after the option's name, which stays its words alone
  (#6075).

- **↑ and ↓ at the edge of a wrapped line stay in the box** (#6045). In a reply box in a list's
  row, ↓ with the caret at the end of a line its words wrap from (as End puts it, in a word too
  long for one line) left the row for the one below. It now moves the caret on to the next line,
  and ↑ and ↓ leave the row only from the box's first or last line.

- **The window no longer waits while purlis reads what is asked of you.** The list of asks is
  read away from the window, so many personas or a slow disk no longer hold it up, and the
  picker on a run's notice reads your settings and vaults once, not once for each entry (#5582).

- **A pane's note that is shown nowhere else can no longer be closed by mistake.** A note in the
  pane's band with no Dismiss of its own, such as a move you can cancel, used to have a × that
  hid it while nothing else listed it. It has no × now and stays until what it says is no longer
  true. A question you close in the band still stays on ✋ and in the Inbox (#5311).

- **A skill brought back under a new name leaves Deleted.** One whose delete was a commit stayed
  listed after it came back under another name (#5457).

- **The keyboard reaches a taken name's choice.** In an install's review, *Skip it* and *Install
  it under a new name* were out of the Tab order until one was clicked (#5457).

- **Your clicks in a browser's view land where you point.** While you drove a browser whose
  picture was a different shape from its page size, as in a pane larger than the page, a click,
  move or wheel landed lower on the page than the pointer (a click on a search field reached the
  button under it), and the bottom of the page could not be reached. The view now works out the
  point from the picture it drew. The same holds in a browser's DevTools view.

- **A card row's ⋯ opens its menu as a menu button does.** Clicked, the menu opens at the ⋯
  with nothing chosen yet; from the keyboard, on its first item. Either way a screen reader
  hears that it is open, and the keyboard goes back to the ⋯ when the menu closes, not to the
  row.

- **A chat starts faster in a project with many personas.** Freezing each persona's tools at a
  chat's start read every persona's definition several times over; with 100 personas that was
  80 ms of every start. Each definition is now read once (#5486).

- **A chat starts faster in a project with many workspaces.** The list of other workspaces a
  chat is told about at its start counted every memory of every workspace first: 30 ms of each
  start at 1,000 workspaces, and 430 ms with a cold disk. The project index now answers it from
  one lookup per workspace, and is rebuilt once after the update (#5486).

- **A chat that gets none of the plane's skills keeps your own skills' descriptions too.** Claude
  Code's listing limit holds the skills and plugins you installed yourself even in a chat that
  the plane's skills do not reach, as it already did where they do (#5842).

- **A Codex chat is told its whole start briefing.** Codex keeps 10,000 bytes of what one hook
  hands it and cuts the middle, and in an ordinary workspace the briefing was longer than that,
  so a Codex chat lost part of its persona's memory, its sandbox notes or the reports kept for
  it. A Codex chat now gets the briefing from four start hooks: who and where it is, how it
  works here, its skills, and what others said to it. Codex asks once to trust the two new
  hooks. Claude Code's and opencode's briefings are unchanged (#5935).

- **A Codex plugin its `config.toml` turns off is no longer listed as in the chat.** The Skills
  view and the Skill studio fold a plugin whose table says `enabled = false` to one line under
  Left out, "Off in Codex", which says which file to change (#4616).

- **A Codex chat is told its skills whole.** Codex keeps 10,000 bytes of what a hook hands
  it and cuts the middle, and the skills list sat in the middle of the start briefing, so a
  Codex chat could lose some of its skills. The list now comes in a start hook of its own, and
  Codex asks once to trust it. Settings warns when a Codex skill budget is larger than Codex
  keeps (#5841).

- **A control's tooltip keeps its width at the window's edge.** The hint on a button or on words
  near the window's edge, or beside words that run across the window, no longer shrinks to a
  column of words: it flips to the other side, or stands under or over its target, at its full
  width (#3364).

- **The Curate menus no longer take seconds on a big project.** Their actions were read again for
  each workspace and persona; with 1,000 workspaces the window waited 1.5 s, and 35 s while the
  project opened. They are now read once (#5486).

- A deleted skill no longer counts as installed: its source's page offers it again, and it stops being offered updates.

- **Dispatch commands no longer say "the purlis app did not answer" while the app is only
  busy.** A list, tell or note used to type queued messages into other chats' panes before it
  answered. Under load that took longer than the command waits, and the message stayed queued.
  The answer now goes back first, and the lines are typed after it. When an answer does run out
  of time, the command says the app is running but busy and names how long it waited. A closed
  connection and an unreadable reply are each said in their own words (#5772).

- **Each sandboxed Claude Code chat has a temporary folder of its own.** Every Claude Code chat
  on a machine used to write its temporary files, its scratchpad and its commands' `$TMPDIR` in
  the one shared `/tmp/claude-<uid>`, so a file one chat wrote could replace another chat's file
  of the same name. Each chat now gets a folder of its own, which its sandbox lets it write and
  no other chat may, and the folder is removed when the chat ends. Codex and opencode chats
  already had one.

- **Code in a diff reads at 4.5:1 in Purlis and Purlis Classic**, on a changed line and on a
  changed word, in every appearance. A changed word's tint is a little lighter in Purlis.
  Comments and punctuation are one grey brighter in Purlis's dark and light appearances, as in
  high contrast. Purlis Classic's code colours step a little brighter in dark and a little
  darker in light.

- **Ending a chat ends the background shells it started.** Claude Code runs each shell in a
  session of its own, so a background shell outlived its chat when purlis quit, restarted it,
  stopped it or settled a crashed session host, and ran on unseen. purlis now ends what a chat's
  harness started outside its process group along with it (#5749).

- **A login follows the same value rules as a fill** (#5272).

- **The save status in the title bar reads only what changed, on a big project too.** git's
  untracked cache was never used for it, and a project purlis had not saved yet had no cache and
  no file monitor token, so each look read the whole tree. On a project with 200,000 files a look
  took 4 to 25 s; it now takes about a third less where git has no file monitor, and less again
  where it does (#5486).

- **The Personas view's In this workspace keeps who worked there after a relaunch.** A persona
  whose chat closed with no session record moved back to On the plane when purlis restarted; the
  project's reopen record now keeps it (#3593).

- **The Inbox and the Notices update their ages with the rest of the window.** Each one kept a
  timer of its own and redrew once a minute on it, separately from everything else and even
  while the window was hidden. Now an age like "5m" changes in the same redraw as every chat
  row's time, and not at all while the window is hidden. It is right again the moment the
  window comes back.

- **A chat starts faster wherever the project index is used.** Checking that the index was built
  with this version's secret rules compiled every rule's pattern first, at every chat start. It
  now reads the rules' names alone (#5486).

- **Keep blocked holds at once.** Keep blocked on a vault, pressed on its Notice, in the Inbox or on
  a run's own notice, takes back the chat's Run grants on it at once, even while another answer
  waits on the session host, and each grant it takes back is audited. From then on no answer
  gives the chat that vault or lets its run go on. An Always allow pressed for that chat does not
  give the persona the vault once Keep blocked is pressed, unless it was already giving it, and a
  tool line it wrote already stays, as the error says. The Allowed list reads again when Keep
  blocked takes back a chat's Run grants, or a run's notice gives one. Keep blocked through a
  vault's own notice no longer logs a warning about asks it closed (#5617).

- **Keep blocked on a vault the persona can already use says so.** If the persona was given the
  vault first, by Always allow or the refused vault's Allow, or the vault is tagged for it, Keep
  blocked does not stop the chat's runs on it. The Notice now says so and where to stop them
  (the Allowed list, the vault's tag or the persona's tool line), and the chat is told not to use
  the vault unless you say so, instead of "kept blocked, don't retry" (#5801).

- **Keep blocked on a run's notice says when it can't stop the chat.** If the chat's persona
  already holds the vault, from its tag or the Allowed list, Keep blocked on a run's notice now
  says what the vault's own notice says: the chat is told its runs are not stopped and not to use
  the vault unless you say so, and the Inbox says where to stop them until you dismiss it.

- **A dispatched task held on an approval prompt says so.** Its row in `purlis dispatch list`
  used to read only "waiting on the person", which a coordinator could not tell from a question
  already answered. It now reads "waiting on the person to allow a tool call in its pane; it
  reads messages sent to it once that is answered" (#5771).

- **A sandboxed chat's `purlis worktree add` no longer tells it to enter the new folder.** Its
  sandbox cannot write there, and the advice sent a dispatched task to an approval prompt nobody
  was watching. It now says the folder is out of its reach, and names what works: open a chat on
  the folder, or have the chat that asked dispatch a task with `--in worktree` (#5770).

- **A LIVE workspace's files say what git will actually commit.** *This machine only* and *Saved
  with the project* are now git's own answer, so a file git still tracks is marked saved, and a
  nested `.gitignore` or `info/exclude` is honoured. A saved name spelled in decomposed Unicode,
  as names from an older Mac disk or an archive are, is now committed. Removing a workspace drops
  what it saved with the project, so a new workspace of the same name shares nothing you did not
  pick.

- **On macOS, a chat's notification now changes in place.** When the chat asks again, its
  notification in Notification Center is replaced with the new one, and it is removed once you
  have answered all of the chat's asks. Before, an old notification stayed until you cleared it
  yourself. The first time purlis sends one, macOS asks whether purlis may show notifications.

- **Listing a memory store looks at each file once.** Every recall that reads the files, and the
  project index's catch-up when purlis starts, asked the disk about each memory twice; on a
  project of 200,000 memories the catch-up took about a quarter longer than it needed to (#5486).

- **A branch folder whose commits merged in a pull request now counts as finished.** When a pull
  request got more commits after a folder's tip and then squash-merged, removing the folder
  was refused, listing commits "that exist nowhere else", although they were part of what
  merged. Now `purlis worktree remove` and `purlis worktree prune` ask the forge. If the
  newest pull request on the branch merged and its head holds the folder's tip, the folder goes
  without `--force`, and its branch with it. If the forge cannot be asked, for example
  offline, the folder stays and the refusal says why (#5439).

- **A branch whose folder is already gone is no longer left behind.** `purlis worktree prune`
  now also deletes each branch purlis cut whose folder was removed some other way, when the
  branch is on its base, carried there by a squash merge, or merged in a pull request. It keeps
  every other branch and says why. `purlis worktree remove --delete-branch` on such a folder deletes the
  branch, or says why it stays, instead of a sentence pointing at a folder that is gone. From a
  chat the app deletes the branch, so its section of the clone's git config goes too (#5439).

- **Move into repos/ survives a crash partway.** A move stopped by a crash, a kill or a power cut
  is flagged by the doctor and finished by the next move, which repairs the branch folders it
  left. A chat with no folder holds the move back, as does one opened while it runs; a folder
  named like one in repos/ in another case is reported as beside it; and the move takes only the
  repos its confirmation showed (#5569).

- **A note you put away in a tab window stays away in the others.** A send that was not
  delivered, a move's line, a "waiting on you" line you hid and "Draft ready in Skill studio" are
  now put away in every window of the project, so none comes back when its chat moves back to
  another window. A tab torn off also takes the "started by hand" banner and a failed restart's
  line with it, where before they stayed in the window it left (#5363).

- **A sandboxed opencode chat on a profile purlis made keeps its data in that profile.** Its
  sessions and logs are written, and its settings and sign-in read, in the profile's own folders,
  inside purlis's sandbox, and the chat stays out of opencode's default folder and every other
  profile.

- **An ended task's `↳ from` goes to the chat that asked it.** It was words alone once the task
  ended. It is now a press, and a row in the task's ⋯ and the palette, while that chat is open
  (#5366).

- **A theme that fills a tree or a table is held to what stands in its rows.** A tree's or a
  table's row draws no fill of its own, so the parts check never measured what is drawn in it on a
  fill a theme gave it. A badge, a count or a field's error in such a row is now read on that fill,
  and a fill they can't be read on draws the row with no fill instead (#5863).

- **A plane's root is never left part-way by a save.** Once a merged request's newer saves are
  settled, they go onto the target as one commit made off to the side, and the plane moves onto
  it in one step. purlis no longer rebases them in place, so a conflict in a generated file no
  longer blocks saving, and a quit no longer cuts a rebase off. A rebase purlis started and a
  quit or crash cut off is recovered by the window's next look or save, with everything chats
  wrote since kept. A stop you made yourself stays yours. Quitting while a save writes the
  plane's git waits for it, and never ends it part-way (#5824).

- **Two machines that each remembered something or allowed a host no longer stop the plane's
  save.** A memory index is rebuilt from its memories when the other machine's saves come in, and
  the project's allowed hosts and browser sites merge as sets, so a host either machine removed
  stays removed. The rest of the project's settings file merges setting by setting: one only
  the other machine changed is taken, and one both changed is named in the Saving view, beside
  the file (#5824).

- **A chat's pool and secret asks wait for you on the chat's own pane too.** A browser login or
  one run of a tool that a chat on a pool's runner asks for, a key a chat's run is missing, and a
  run of a program its persona does not declare are each drawn on the chat's pane, as well as in
  Asks. A login or a tool's run is answered with its own buttons there; a secret's Answer opens
  its fields and its Allow under the line. Before, a person looking at the chat, with the right
  side on another view, saw nothing to press, and the chat waited.

- **`purlis worktree prune` from a chat now answers on a large workspace.** On about 290 branch
  folders it gave up after two minutes with "os error 35" and printed no list, though the app
  went on and finished the prune about 20 minutes later. A chat now waits for as long as the app
  is still working through folders, and the app reads several folders at once, so the same
  prune takes a fraction of the time. If a chat does stop waiting, it says plainly what was
  removed: nothing for a dry run, or the folders of the workspace removed since it asked
  (#5439).

- **The push scan no longer refuses a save over numbers purlis writes itself.** A card number,
  or a national ID's grouped form such as an Aadhaar number, is not read where it is part of a
  file's or branch's name (a session record named for the issues it closed), part of a decimal (a cost in
  dollars), or written after `#`. A card or ID number written in a sentence, or an
  ID behind its name (`Aadhaar: …`), is still caught (#5822).

- **A merge of a branch from before the scan's allowlist existed is no longer refused as an
  allowlist change.** Only a commit that changes the allowlist, or a merge that writes one
  neither side had, is. The refusal now names `.purlis-scan-allow.toml` (#5822).

- **A push from a bare repository to the remote's address leaves out what the remote already
  has**, as a push from a clone does, instead of scanning the whole history again (#5822).

- **A chat that names its conversation as it starts can always be restarted.** A Codex or opencode
  chat whose first report came while purlis was still starting it lost its conversation for good:
  every restart said it had none to resume, and the next launch could not resume it either (#5170).

- **Restart now on a chat waiting on its own background work goes ahead and tells the chat.**
  Restart now, a restart without the sandbox and a restart for changed project settings end the
  chat at once, and the chat is told in its first message how many shells, monitors, subagents
  and workflows the restart ended. Any other restart of such a chat waits until its work wakes
  it, as the window's already did. After the hour's wait lapses, the chat still shows as needing
  you, and a restart or a move to a runner goes ahead and tells the chat what it ended (#5753).

- **A chat that dispatched tasks is told about them after a restart.** Before, a chat brought
  back at launch was told nothing about the tasks it had dispatched, and a report waiting for it
  sat there until the person typed. Now its first message after the restart lists each task by
  chat number and says where it stands: brought back and carrying on, still running, or ended
  because purlis could not bring it back. The message also names each task whose report waited
  for the chat, and the report reaches the chat as context (#5754).

- **`purlis dispatch wait` returns a report the app no longer holds in memory.** After a
  restart, or after an earlier turn had read the report, it used to say the app "no longer holds
  its text". It now reads the report from the task's dispatch record (#5754).

- **After a quit, a chat is told what its background shells last printed.** Its first message
  after the quit used to say "output gone" for each shell and monitor, because the chat's own
  temporary folder, where that output is written, went with the chat. A chat that ends with work
  of its own now keeps the folder until its next run ends, or until it is closed (#5827).

- **Every reopen tells a chat what the restart ended, not only a quit.** A chat waiting on
  work of its own now gets its wake line when the session host crashed and Resume starts it
  again, whether it was idle or mid-turn, and when a chat the launch meant to reattach has to
  be started after all. The host-crash notice names what ended (for example "1 background
  shell") and no longer says "nothing was lost" for such a chat. An update, a window crash past
  its grace, Reopen all and Try to start again tell it as a quit does. A chat the session host
  kept running, and a chat Start fresh does not put back, are told nothing (#5752).

- A chat that started background work in a turn that purlis's quit or update cut off is now told
  about that work when it is reopened. Each background shell, monitor, subagent, workflow, cron
  and wake-up is recorded when its tool starts it, not only when the turn ends. The wake line
  also says that the chat's last turn was ended mid-way and names each piece by what it was.
  `/clear` clears the record, so a cleared chat is never told about work from its old
  conversation.

- **Quitting or restarting to update warns about a chat in a background turn.** A chat working in a
  background turn over its wait on you used to look idle to the quit warning and the update
  dialog, so neither said that quitting or restarting would interrupt it. Both now name it as
  mid-turn (#5782).

- **Start fresh warns about a chat waiting on its own work.** Its question now says which of the
  chat's background shells, monitors, helpers and wake-ups will end, as the quit warning does.
  It also warns about a chat in a background turn (#5782).

- **A chat waiting on its own background shell is told when a restart ended it.** A quit or an
  update ended the shell along with the chat, and the chat brought back at the next launch
  waited for it for ever. purlis now records what each chat left running of its own in the
  record of open chats, every time that changes. When the chat comes back, it is told in one
  line how many shells, monitors, subagents and workflows the restart ended. A task that still
  owes its report is told to carry on in that same line. A chat the session host kept running
  is told nothing (#5747).

- **A row's own controls keep their keys in the Inbox and every list of rows.** Esc that cancels
  a Chinese, Japanese or Korean composition in a reply box no longer takes the keyboard out of
  it; ↓ on a select in a row opens it instead of moving to the next row; and ↑ and ↓ in a reply
  whose words wrap move a line at a time, leaving the box only from its first or last line
  (#5993).

- **A row's tooltip can be hovered, and comes back after Esc.** The pointer can move onto a
  list row's tooltip to read it without it closing, and a tooltip Esc took down shows again the
  next time the pointer rests on the row or the keyboard comes to it (WCAG 2.2 SC 1.4.13, #5038).

- **Keep blocked on a refused vault holds against a run already on its way.** A run that
  passed the check just before you pressed it no longer raises a notice after it, and the
  press no longer waits on the disk on the window's thread (#5424).

- **A clone a runner's chat made comes back without a second copy or a stuck move.** Two clones
  started at once in a workspace with no `repos/` no longer fail because one took the folder away
  as the other used it. A clone the runner's chat made is no longer copied into `repos/` beside
  an old clone of the same name at the workspace's top, whether the names differ only in case or
  the old clone's `.git` is a link. An old clone named `repos` no longer stops a chat's work from
  coming back: only the runner's new clones stay on the runner, and the chat's tab says to rename
  that old clone so they can come back.

- **A clone a runner's chat made comes back into `repos/`.** Moving a chat back from a runner now
  makes its new clone at `workspaces/<ws>/repos/<name>`, like every other new clone, and a folder
  of yours with that name at the workspace's root no longer stops it. `purlis clone repos` finds
  an old clone named `repos` instead of refusing it, and a clone that fails no longer leaves an
  empty `repos/` folder behind (#5534).

- **A second session host no longer starts beside one whose lock file the system cleaned away.**
  The host and the window also find a host serving by its socket.

- **A GitHub App token is never kept for a chat that ended while its run was being served** on
  this machine.

- **Claude Code chats are held from writing a program held by its own name**, as wrapped chats
  already were.

- **The save status is right on a very big project on a slow disk.** On a project where
  reading every file takes git more than 30 s, purlis stopped each status at 30 s. The status
  bar then showed nothing to save, even when there were changes. The one status that prepares
  git's caches now gets two minutes, so it finishes once and later reads are quick (#5486).

- **A Secret ask's record keeps its order.** An ask raised before the event log opens is
  recorded as it opens, so its answer is never there without it. A Run grant's raise is in the
  chat's trace before its answer (#5509).

- **A session host that stops answering no longer piles up secret asks' steps in memory.** At most
  512 wait on it; a step past that is not written to the event log, and takes its ask's later
  steps with it, as a raise the log refused does. The chat's trace keeps the whole of the ask
  (#5618).

- Secret fill: what the person typed while driving a browser no longer stops a later fill in it.

- Secret fill: a site whose name holds a login's user name stays visible to asks, and can be filled, once that value has been filled on that site.

- A page on the site a browser signs in on is told as one even where its address holds a value kept out.

- More spellings of a kept value are taken out of what a chat reads: letters whose capitals are spelled differently, a Latin-1 address, and a `+` read as a space.

- Secret fill: no fill goes to a browser run by an older purlis on a runner or host. The chat is told to update it.

- **A custom theme's switcher rows and plain count are read as they are drawn.** A popover filled
  opaque is read in the workspace tints when a switcher row on it draws a see-through fill, so its
  words can no longer fall in a tint unseen; and a see-through rule on `activity.count`, or a
  see-through `needs-you.count`, is held apart from the hot count as the two come out on the bar,
  not as they are written (#4607).

- **Shift+Tab back into a list lands on the row you left, not on a button or box in it** (#6033).
  In the Inbox and every list whose rows hold controls, a row whose controls came last in the
  list used to give Shift+Tab to its last control, so a screen reader announced that control
  with no row around it. Shift+Tab now lands on the row itself, and → reaches its controls
  again.

- **An ended task in a tab behind the one in front is offered Go to … in the palette**, while the
  chat that asked it is open, as one in the tab in front already was (#5543).

- **Set up signing's Check no longer tells you to add a key that's already on your account.**
  GitHub matches a signed commit to an account by the commit's email, so when the email isn't one
  of the account's verified ones it says it doesn't know the key. Check now sees that the account
  lists this device's key and says the email is why, then offers the account's verified emails to
  sign chat commits as. Check also always names the email it committed as.

- The Skill studio no longer says a skill's script calls a program because the word appears inside
  a quoted string or a here-document, such as `sort_by` in a jq program or `body{font:…` in CSS.
  A program the script really runs is still named, also inside `$(…)`, `bash -c '…'`, `eval` and
  a here-document fed to a shell (#5742).

- A skill run's chip on a pane head is named by the words it shows, so a screen reader and voice control both find it by what's on screen (#5614).

- **Smart close and the memory skill work in a sandboxed chat.** They fold lasting decisions,
  glossary terms and a changed goal into a workspace's charter through the `workspace_section`
  and `workspace_vision` tools, instead of telling the chat to edit `workspace.md`, which the
  sandbox refused (#5720).

- **A chat's ssh connects on Fedora and RHEL.** Its route ran the system's `nc` with OpenBSD
  netcat's proxy flags, which ncat refuses, so ssh never connected there. The route now carries
  ssh through the chat's SOCKS port with purlis's own command line, so it needs no `nc` on any
  system, and where none is found ssh says why (#3536).

- **The Start dialog's Secret ask holds every start while you answer it** (#5670). While Fill or
  Not now is sent, Return from the persona list, ⌘↩ (Ctrl+Enter elsewhere) and Start start
  nothing, and Start no longer says "Starting…" for a Not now. ⌘↩ in the value's box fills. The
  ask is drawn only while the profile it was refused on is picked: pick another and it goes, pick
  that one back and it returns, and a fill sent while you picked another profile starts nothing
  and says whose key it filled.

- **The Start dialog says to type a refused value again.** When you fill a start's missing
  credential and purlis refuses the value you typed, for example because the vault is locked, the
  dialog now says to type it again and press Fill, as it does when the answer could not be sent.

- **The status line answers faster on a big project.** Its check for tracked changes at the
  project root also listed every untracked file, which it then ignored. It no longer lists them
  (#5486).

- **One Tab leaves a list from its row, and Tab walks a row's buttons both ways** (#6071). In the Inbox and every list
  whose rows hold controls, one Tab from a row leaves the list, past the row's buttons and boxes,
  as one Shift+Tab does. From a button in a row, Tab now moves on to the row's next control and
  Shift+Tab back to the one before, as they already did from a box; past the last or the first,
  both leave the list.

- **Faint text in the terminal is readable in every appearance.** Words a program draws faint
  in the terminal's own colour now take the theme's faint colour, which holds 4.5:1 against
  the terminal's background, instead of being drawn at half opacity. Themes can set it with
  the new `terminal.faint` token, and an imported theme gets one made from its own colours.

- **The ✋ no longer counts a chat that isn't waiting on you.** A dispatched task's turn end waits
  on the chat that dispatched it, so it is no longer listed in Asks as "Waiting on your reply",
  unless the task asks you directly (a permission, a secret, a sandbox block) or that chat has
  closed. A chat whose turn ended while its own background command or helper still runs is no
  longer listed as waiting on your reply because an update about it was already in the Inbox.

- **The Skill studio shows one copy of a skill you have two of, as your chats do.** Where a project
  skill is the same, file for file, as one a program finds itself, the studio's preview of that
  program's chats leaves the project's copy out. Harness skills lists it under the program as
  _Same as its own copy_, naming the copy kept, and `purlis skill list` says so too (#5843).

- **`purlis theme check` says when a theme's side-view cards blend into their panel or a text
  box loses its edge**, even when the theme sets no component parts. A card row has to stand a
  step from its panel. A text box's edge has to hold 3:1 on the window, a panel and a dialog.
  Code has to read at 4.5:1 on the editor's own background. The check only reports these.
  Purlis draws the theme as written.

- **Text boxes have a visible edge in Purlis Classic**, at 3:1 or more on every ground they
  stand on.

- **`purlis theme check` says when a theme's colours lose a meter's bar on its track**, even
  when the theme sets none of the meter's parts: the score, the baseline and the previous version,
  each at 3:1 on the track. The check only reports it. Purlis draws the theme as written.

- **Tab leaves the title bar's pinned workspaces from the chip you arrowed to.** After the arrow
  keys move along the pinned chips, Tab goes on past the strip and Shift+Tab comes back to that
  chip, not to the one the keyboard entered on (#5912).

- **Undo a turn and Discard keep what the file was.** Overwrite on a discard keeps a file's CRLF
  line ends where the repo converts them, instead of writing every line with LF. Undo puts back
  an executable bit the turn changed, and is refused when a file's mode changed since the turn
  ended. Choosing a turn whose diff could not be read no longer offers Undo of the turn shown
  before it.

- **The save status in the title bar no longer reads every file twice on a big project.** Its
  check for a merge stopped part-way looked at every tracked file on disk, when git's index
  already says which files are in conflict. On the 200,000-file test project it was 24 s of each
  status on macOS and 1 s on a fast Mac; it now reads the index alone (#5486).

- **Closing a tab behind the one in front leaves the keyboard where it was.** Its × no longer
  takes the keyboard on its way out, so it stays in the front tab rather than landing on the
  window (#5445).

- **A narrow pane's head draws a chat's state once.** Under about 470px the head drops the
  state's word, and no longer draws its glyph a second time beside the chat's mark, which wears
  it already (#5446).

- **A folded pane that needs you shows one ✋.** Its rail drew its own ✋ beside the chat's
  mark, which wears one already; now the mark's is the only one (#5447).

- **In Classic, the × of a tab under the pointer stands on the tab's own hover colour,** not on a
  lighter box (#5524).

- **A screen reader finds its way round the window by its landmarks.** The title bar is the
  window's banner, the work under it is the main landmark, and the project rail is a navigation
  landmark named Projects. Before, the whole window was one main landmark, title bar and all, the
  rail was in none, and a side's view was a landmark inside its side's own, so it was listed
  twice.

- **A write to your workspace files lands in the folder purlis checked.** New file, new folder,
  rename and import keep the folder they checked open until the write. If a chat swaps that folder
  for a link in the meantime, nothing reaches the link's target. A delete is refused if its folder
  changed after the check. Renaming a file onto another hard link of itself is refused and no
  longer reports a success that did nothing. Importing a FIFO is refused at once rather than
  hanging. An import that stops partway names the files it already copied. A rename is refused on
  a disk that cannot rename without the chance of replacing a file (NFS, some FUSE and SMB mounts).
  On Linux, a delete on another disk than your Trash goes to that disk's own Trash, and is refused
  only when that disk has none purlis can use, since the system would then copy it into yours and
  lose its permissions and links (#5551).

- **A branch folder whose pull request closed with its head on origin is removed without
  `--force`.** `purlis worktree remove`, `worktree prune` and a task folder's removal let it go,
  and where that head is on no branch of origin the refusal says the pull request closed. A
  refusal now names every reason at once (commits, uncommitted changes and ignored files), and
  `.astro/`, `.vite/` and a `node_modules/` holding only Vite's cache no longer keep a folder
  (#4500).

## Security

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
