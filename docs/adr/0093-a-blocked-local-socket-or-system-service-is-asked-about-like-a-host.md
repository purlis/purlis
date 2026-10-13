# A blocked local socket or system service is asked about like a host

**Accepted 2026-10-12. Amended 2026-10-13 (operator rulings R1, R2 and R3, at the end): the
certificate check is on by default and no longer a grant; a socket or a service may be allowed
for every project on this machine; what a project declares it expects is asked about up
front.** Amends ADR 0067 §2 ("each compiled profile allows the chat to connect to
that socket and nothing more") and the ruling it carried in the sandboxed chat's briefing,
"purlis grants no local socket" (#1779).

## The operator's words

> Solution should be not only for docker, it should be in general, as other apps also can have
> this cases, and solution should be most idiomatic overall.

Then, the same day, three rulings:

1. **Docker and Podman sockets get every option**: Allow once, Allow for this chat, and Always
   allow. The Notice carries the warning "about the same as no sandbox". No special case beyond
   that warning.
2. **A project's committed settings may declare the sockets and services it needs**, the way a
   Flatpak manifest does. The declaration only shapes the ask's wording and the Settings list.
   It never grants anything: each person still allows on their own machine, and grants stay in
   the local file.
3. **The certificate check (trustd) is ask once, then always.** The first time a Go tool's TLS
   fails, the Notice offers the grant and states the trade-off. Always allow is offered. Nothing
   is granted by default.

## The decision

A sandboxed chat's sandbox still refuses every local socket but the app's own hook socket, and
every system service by name but the few every program needs. What changes is that **a refusal
of one never dead-ends**: it gets the grant flow hosts and folders have, as Flatpak permissions
and Apple's app sandbox do.

- **The grant units** (D-sock-1). A local socket, by the path the kernel names it by, links
  resolved, so a link a chat puts in its own folder later widens nothing. A system service
  (macOS `mach-lookup`), by its whole name, never a prefix. The keychain's services are never
  granted (ADR 0067 §5 class 1), and no socket under a path the sandbox denies by class is.
  **No socket in a folder a chat may write** is granted either (the project tree, the user's
  temp folder, each harness's own folders, every folder a person let chats write), nor one with
  a second hard link, nor one another user owns, nor one in a folder others may write that is not
  the person's own (`/tmp`): a chat, or anyone, could otherwise put any socket there under a
  harmless name. Each
  is judged by its resolved path when offered, when allowed, and again at every start; a kept
  path that is now a folder, a file, a linked socket or somewhere a chat writes is dropped. A
  container engine makes its socket anew each time it starts, so a grant is of the path, not
  the inode.
- **Where a grant is kept** (D-sock-2). For one chat, in memory, as a host's is. For this
  project on this machine, in the person's own config home,
  `~/.config/purlis/sandbox-grants/<clone key>.json`, which no sandboxed chat writes and no
  repository brings. **Never in the project**: a repository could grant itself a socket that
  way. So no "everyone in the project" level is offered.
- **Every per-machine key moves with it** (D-sock-9). The record an older build kept in the
  project (`app/sandbox.json`: folders and hosts of yours, vaults, dispatch grants, and the
  acceptances `hosts_seen`, `presets_seen` and `dispatch_*_seen`) is now kept in that file too,
  so the project holds nothing that grants or accepts. **The older file is never read**: nothing
  in a project can tell a copy that came with the repository, a zip, a synced folder or a `.git`
  that came along from this machine's own, so after this update each project's grants and
  acceptances are asked about again, once. The clone key is the clone's real path, the folder's
  own identity and its `.git`'s, so a folder made again or a repository cloned into the same
  place later inherits nothing.
- **Allow once** (D-sock-3). A socket or a service reaches a chat only through the sandbox it
  starts with, so a grant takes effect when the chat restarts on its conversation. "Once" is
  that run alone: it goes when the session it was allowed in ends, which the restart that takes
  it outlives, so no later start of the chat has it. It is offered for a socket or a service
  only: a host is asked about live, and a folder is written for longer than one run.
- **A declaration** (D-sock-4). `[sandbox].expects` in the committed file lists the sockets and
  services the project expects. It changes the words of the Notice ("This project expects it.")
  and marks the grant in Settings' Allowed list. It grants nothing; a test plants one and checks
  that nothing is compiled from it.
- **Detection** (D-sock-5). Claude Code's `<sandbox_violations>` lines and purlis's own Seatbelt
  log name the socket; so does a **failed** command's own error (Go's `dial unix <path>: connect:
  operation not permitted`, Docker's sentence on macOS), the one place a client says the path.
  What a command that succeeded printed (`cat` of a log, a file that quotes the words) raises a
  block that names no socket, so no Allow. **A line that runs a container engine's client is
  read as one that failed** (2026-10-13): a chat runs `docker run … 2>&1 | tail`, the pipe ends
  with `tail`'s status, and the harness hands the result over with no failure beside it, so
  the refusal named no socket and its Notice had no Allow. No harness says how each program of
  a line ended, so the one signal left is the line itself: where one of its own programs is
  `docker`, `docker-compose`, `podman`, `podman-compose` or `nerdctl`, by name, after the
  shell's own plumbing (assignments, redirections, `timeout`, `env`), the socket its output
  says it was refused is named. Such a client prints its own error or what a daemon it already
  reached sent it, and a line that only reads (`cat`, `grep`, `git show`, `curl`) still names
  none, nor does a string a shell is handed (`sh -c '…'`). The line is read for that one
  answer and nothing of it is kept. A `mach-lookup` violation names the service; Go's
  `x509: OSStatus -26276` names the certificate check. The Notice shows the path whole and
  judges it again as a grant before anything is kept. **The certificate check is named
  whenever the line runs a program** (2026-10-13, to be confirmed): `gh pr list … 2>&1 | head`
  ends with `head`'s status, and its Notice said it was the certificate check and offered no
  Allow. The check is one service and its name is purlis's own constant, so nothing a chat
  printed is ever put in the Notice by naming it. What stays ruled out is a result that only
  read the words: a line whose every program only reads or prints (`cat`, `grep`, `sed`, `git
  show` and the like, also inside a string a shell is handed), a file tool's read, and a result
  with no command beside it name no service. A here-document's body is text its opener reads
  and is never read as a line; a result that holds both streams in one text keeps to the same
  rule. Any other program may be the one that was
  refused: no list of the programs that check certificates could be kept whole. This rule
  holds for the certificate check alone, the one kind with a single service; a socket
  and any other service keep to the rule above. (Since R1, below, naming it offers no Allow:
  the Notice says what stands for certificate checks in the project and the way on.)
- **Honest words, per known kind** (D-sock-6). The Notice and the ask say what allowing it means:
  - Docker and Podman sockets: lets this chat control containers, which can reach your whole
    machine: about the same as no sandbox;
  - ssh-agent: lets this chat use your SSH keys;
  - database sockets: lets this chat use that database as you;
  - trustd: lets Go and similar tools check certificates; that service can make its own network
    requests;
  - anything else: a service on this machine; purlis can't tell what it does.
- **Per harness** (D-sock-7). A socket's Allow is offered on macOS only, where it compiles.
  Claude Code takes a socket in `network.allowUnixSockets` (on Linux its seccomp filter cannot
  tell sockets apart, and purlis wraps no harness there yet), and of every system service only the certificate check, as
  `enableWeakerNetworkIsolation`; any other service's Notice offers Start without the sandbox
  and says why. purlis's own wrap (Codex, opencode) takes a socket as a connect-only
  `unix-socket` rule and a service as a `global-name` lookup.
- **Policy** (D-sock-8). An administrator's `socket-grants: false` forbids every socket and
  service grant and takes away any made, the certificate check included, the committed
  `certificate-checks` too.

## Amended 2026-10-13: the person names what the sandbox did not (#1884)

**To be confirmed by the operator.** This extends D-sock-5 within its intent, that a refusal
never dead-ends and the person names what is allowed; it changes none of the rules above.

D-sock-5 names a socket or a service from the sandbox's own line, a failed command's error, or
a container client's line. Where none of those is there (a build runner that starts Docker with
its status dropped, a background command read back later, the certificate check down a pipe),
the block reached the app naming nothing, and its Notice said there was nothing to allow.

- **The person chooses** (D-sock-10). Such a Notice offers **Choose the socket…** (or the
  service) beside Start without the sandbox. It lists what **the app itself** finds on this
  machine: the places known services keep their sockets, a fixed table, and what the app's own
  environment names (`DOCKER_HOST`, `CONTAINER_HOST`, `SSH_AUTH_SOCK`, `PGHOST`,
  `MYSQL_UNIX_PORT`, `XDG_RUNTIME_DIR`), each kept only where it is a socket a grant may name
  now. No file is read and no program is asked. A socket may also be typed by its path. A
  service is chosen only from the ones purlis can allow: a service has no path to check, so
  none is typed. **Since R1 (below, the same day) that list is empty**: the certificate check
  was the one on it, and it is a setting now, never a grant. A refused certificate check that
  names nothing gets the Notice a named one gets, which says what stands and the way on.
- **Nothing a chat wrote is in it.** The list is the app's own, in the app's order; the block
  carries no target, so the box starts empty. The list, the choice and taking up a read-back
  block are the window's alone (`WINDOW_ONLY`).
- **A chosen target is a named one.** It is judged by the function that judges a socket the
  sandbox named (D-sock-1): a socket on this machine, of one name, where no chat writes, under
  no denied path. It gets the same words for what allowing it means (D-sock-6), the same
  policy and harness rules (D-sock-7, D-sock-8), and the same three choices (D-sock-3). The
  grant is judged again when allowed and at every start. Choosing grants nothing.
- **A socket named and refused may be chosen again**: where the one the sandbox named is in a
  folder a chat may write (Podman's in the temp folder), the Notice keeps that sentence and
  offers the choice, so the person can pick the socket the service keeps elsewhere.
- **A block a chat only read back asks nothing** (D-sock-11). A block found by a tool that ran
  nothing (the file tool, or the harness's reader of a background command's output) is a
  quiet line: it says where it came from, no ask lists it, the chat is not told to wait, and
  Dismiss puts it away. **Show what you can do** takes it up, and it is then the block's own
  Notice. A command the chat runs is read exactly as before, and its block is never held back
  behind the quiet line of a read.

## The committed `certificate-checks` (a behaviour change)

**`[sandbox].certificate-checks` now declares the certificate check and grants nothing**
(decided under the operator's rulings 2 and 3: committed files declare, never grant; the
certificate check is ask once, then always). A project that sets it gets the Notice's words
"This project expects certificate checks", and each person allows it once on their machine the
first time a Go tool checks a certificate, Always allow among the choices. Projects that set
it now get one Allow per machine the first time a Go tool checks a certificate.

## Allowing the certificate check is one press, and said where the person looks (2026-10-13)

The first day of the change above, the person who made the rulings had `certificate-checks = true`
in two projects, saw the switch on in Settings, and had `gh` fail in every chat. The Notice's main
button was Allow once, so each chat asked again; and "Allow for me on this machine", pressed while
the chat was mid-turn, was kept and changed nothing they could see until that turn ended. Nothing
granted wrongly, and nothing here moves a ruling: the committed file still declares and never
grants. What changed is that the Allow is easy to make and its state is said:

- **The certificate check's main button is "Allow for me on this machine"** (ruling 3: ask once,
  then always). Only this chat, and once, are the narrower choices beside it. A container
  engine's socket and a service purlis cannot tell keep Allow once first.
- **Settings says what a chat here has, and allows it.** Under the committed switch, a row
  "Certificate checks on this machine" says whether the person allowed it here, in words that
  name what fails until they do, with **Allow on this machine**: the same grant the Notice
  makes, kept in the same file, audited, the window's alone, refused under `socket-grants:
  false`. A project need not wait for a tool to fail first.
- **A block of what is allowed already says so.** A socket or a service the person allowed for
  the project on this machine is refused still to a chat that started before the Allow. Its
  Notice says the Allow stands and offers Restart this chat, never the same Allow again.
- **A restart that waits for a turn says so.** A grant reaches a chat when it restarts, and
  purlis never ends a turn by itself. While a chat owed a restart is mid-turn its pane says the
  restart waits for the turn to end, with Restart now, which ends the turn.

## A block is learned from the sandbox first (2026-10-13, to be confirmed)

The operator's words, the day D-sock-5's detection was patched three times: the dead ends are
"not acceptable — should be fixed idiomatically." Each patch taught the reader one more shape
of what a refused program prints. The cause is the same in all of them: what a program prints
and how its line ends are the program's to choose, and the sandbox knows what it refused
whatever the program does next.

- **The sandbox's own record is read first** (D-sock-12). A denial as the sandbox reported it
  says the process, the operation and the exact target: macOS's kernel line (`Sandbox:
  <process>(<pid>) deny(1) <operation> <target>`), which Claude Code reads for its own commands
  and appends to the result, and a proxy's line for a host. It is one record
  (`sandboxblock::denial`), sorted by the one sorter, and it names the socket, the service, the
  file or the host whatever the program printed or ended with: down a pipe, behind a build
  runner, or from a tool that blamed its own token. Where the same block is also read from the
  program's words, it is one block, and it keeps the name.
- **It only ever adds a name.** The target is judged again as a grant before anything is
  offered, and nothing is granted without the person's Allow. A harness hands its report over
  in the text of a result, so the rules for what a command printed stand: a report in standard
  output, or with the command's own output after it, is the command's. The one thing read past
  is the note Claude Code writes below a failed command's report, by its fixed opening and only
  as one paragraph that ends the text.
- **What a program printed is read only where no record can exist**: a chat purlis wraps
  (Codex, opencode), whose sockets, services and writes no one reports yet; a Claude Code
  command that ended before the kernel's entry reached its harness, or that runs in the
  background; and a lookup past the proxy, which no line names.
- **Whose records** (the operator's ruling the same day). purlis may read the sandbox's denial
  records for its own child processes only: what Claude Code hands it, and macOS log entries
  about its own children. Nothing wider, no administrator's rights, no new entitlement.
- **Not done, and proposed**: purlis reading the kernel's entries for the chats it wraps, by a
  tag only its own profile's denials carry, as its proxy reports hosts. `log stream`, the one
  unprivileged way to read them, runs only on an administrator's account, which is for the
  operator to rule on before it is built.

## Amended 2026-10-13 (operator ruling R1): certificate checks are on by default

The section on allowing the certificate check in one press made the Allow easy. The same day
the person who made the rulings met the
dead end a fifth time: a tool failed with an error that hid its cause (`gh auth status` said
"token invalid"), a Notice appeared or not depending on a pipe and an exit code, and a project
opened on a new machine said nothing until then. His words: "this is not acceptable — should
be fixed idiomatically." A grill followed, and he ruled:

> **R1.** Certificate checks (macOS trustd, `com.apple.trustd.agent` and what the check needs)
> are ON BY DEFAULT for every sandboxed chat. A machine policy lock or a project setting can
> turn them OFF for strict setups.

**This replaces ruling 3 above, and takes the certificate check out of ruling 2's reach.**

- **Every sandboxed chat has the certificate check**, on every harness: Claude Code as
  `enableWeakerNetworkIsolation`, the profile purlis wraps Codex and opencode in as the one
  `mach-lookup` rule, a task's chat as any other. Nothing is asked, declared or allowed for it.
- **It is off in two cases only**: the project's committed `certificate-checks = false`, for
  everyone in the project; and an administrator's `socket-grants: false`, on that machine,
  whatever the project says. One answer decides what a start compiles, what Settings says and
  what a block's Notice says (`sandbox::CertificateChecks`).
- **The committed key keeps its name and becomes an opt-out.** `true` or absent is on. The
  meaning of an absent key changed twice in two days; it is said in `plane-format.md`, and
  nothing is migrated. A value that is neither `true` nor `false` is refused with a sentence
  and read as off.
- **Ruling 2 stands** (R3 of the same grill): a committed file declares and never grants.
  `false` only narrows, and `true` grants nothing a project without the key lacks.
- **The reason.** Developer experience first, then standard practice: nearly every tool
  verifies TLS through this service, a chat without it cannot use `gh`, `glab`, `terraform`
  or `gcloud` at all, and a refusal there reads as a broken token or a broken network. Claude
  Code's own sandbox offers exactly this switch for exactly this reason.
- **The residual, accepted.** The service makes its own requests, for revocation and for the
  certificates a certificate names (OCSP, CRL, AIA), and those do not pass the egress proxy. A
  chat cannot choose their contents or read their answers, but it can cause them, to an
  address in a certificate a host it reaches presents. It is narrow, it is said in Settings
  under the switch, and a strict setup turns the check off.

What follows from it:

- **The per-machine Allow of the certificate check is gone**: Settings' "Certificate checks on
  this machine" row and its window-only `allow_certificate_check` (added earlier the same day),
  the Notice's Allow at every level, and `certificate-checks = true` as a
  declaration. Nothing allows `com.apple.trustd.agent` by name any more, and an Allow of it is
  refused. **A grant an older build kept in `services_mine` is read and changes nothing**, on
  or off: it is not listed in Settings.
- **`false` holds against the person too.** Nothing a person allowed turns the check back on
  in a project that turned it off. (Decided in implementation, fail-closed, and flagged for
  the operator: the other reading lets one press on one machine undo a project's strict
  setting.) Its Notice never dead-ends: it says the project turned it off, where to turn it on
  (Settings › Sandbox, for everyone), and offers Start without the sandbox.
- **Settings shows one switch**, Certificate checks, on by default, with the residual in its
  help. Under policy's lock it is a status line that says who set it.
- **A chat that started before the change** keeps the sandbox it started with. The "Sandbox
  changed" Notice lists it with Restart, and a refused certificate check in it says the
  project has them on and offers Restart this chat.
- **A chat that started with the check and is refused it all the same** is not told to
  restart, which would start it with the same sandbox (review, 2026-10-13). Its Notice says
  the chat has certificate checks already, that something else refused the tool, and offers
  Start without the sandbox as the way to tell. Whether a chat has the check is the app's own
  record of what its sandbox was compiled with, never a word the chat sent.
- **The one-time presets Notice** counts certificate checks among the defaults a machine has
  seen, so a default project tells nothing. A machine that recorded what it saw before this
  change is told once that certificate checks are on, with what that widens.

## Amended 2026-10-13 (operator ruling R2): every project on this machine

> **R2.** Where something stays a per-machine grant (sockets, other services), one press can
> cover this machine for every project; per-project remains under "Other scopes".

This adds a level to D-sock-2's two (one chat in memory; this project on this machine):

- **Every project on this machine**, for a local socket or a system service only. What a
  machine has (a container engine, an agent) is the same whichever project asks for it, and a
  person who allowed Docker's socket in one project was asked again in the next. Kept in
  `~/.config/purlis/sandbox-grants/every-project.json`, beside the files kept per clone: this
  machine's own, which no sandboxed chat writes and no repository brings. **Never a host or a
  folder**, which stay a project's, and never a dispatch grant.
- **Where it is offered.** A block's Notice for a socket or a service draws two answers side by
  side: **Allow once**, still the main button and the narrowest, and **Allow on this machine,
  for every project**. Only this chat, and this project alone, are under Other scopes. (Decided
  in implementation and flagged: the widest answer is one press away and named for what it is,
  and is not the default button, because a container engine's socket is about the same as no
  sandbox, for a project opened later too.)
- **It is judged like any grant**: by the socket's resolved path when offered and when allowed,
  and again at every start **for the project the chat starts in**, so a socket inside that
  project's own tree reaches no chat there. Audited at this level; listed in every project's
  Allowed list as "Every project on this machine", with Remove, which takes it from them all.
- **Policy.** `socket-grants: false` forbids it and takes it away, as it does a project's. A
  policy that fixes `allow-scopes` leaves this level out unless it names it: `you` does not
  stand in for it. (Its word, `machine`, was ruled on later the same day: see "the policy's
  word for every project" below.)

## Amended 2026-10-13 (operator ruling R3): what a project expects is asked about up front

> **R3.** Kept: a committed project file declares, never grants.

With it the operator ruled that what a project declares it expects is asked about up front,
when a chat starts, and not discovered by a failure. A committed file still declares and never
grants (ruling 2), and each person allows on their own machine. What changes is **when the person is asked**. A
project that declared a socket said nothing on a new machine until a tool failed there, with
an error that often hid the cause. Now the declaration is read where the project is opened:

- **One Notice, once**, when a project opens (and when its settings change on disk) and its
  `[sandbox] expects` names something this machine has not allowed, for this project or for
  every project: "This project expects more than this machine allows", with a line for each
  item (its whole name, drawn inertly since a repository wrote it, and D-sock-6's words for
  what allowing it means), and, for one that cannot be allowed here now, why.
- **Allow on this machine** keeps each listed item that can be allowed, for this project on
  this machine, by the route a block's Allow takes: judged, audited, kept in this machine's
  file. Only what is declared, was shown and still waits is kept, whatever the window sends,
  and the answer is recorded for what was shown alone: an item declared since is asked about.
- **An Allow keeps what was shown, as it was shown** (review, 2026-10-13). The question is
  drawn once and may be answered much later, and a declared path can come to lead somewhere
  else in between. So the window sends each item as it drew it, the name an Allow would keep
  included, and nothing for one it drew as "cannot be allowed here now". An item is kept only
  while it still stands that way at the press. One that changed is neither kept nor recorded
  as answered: the Allow says so, and the question is drawn again as it stands.
  **Not now** is remembered on this machine (`expects_answered`); Settings › Sandbox has a
  row, "This project expects", that still offers the Allow. The Notice returns only when the
  project declares something new, or (#1927) when a path it declared comes to name another
  socket: an answer is kept with the name each item was shown under, so "Not now" to one
  socket is not "Not now" to whatever its path leads to later. A path that goes back to a
  socket already answered is not asked about twice.
- **Both answers are the window's alone** (`allow_project_expects`, `decline_project_expects`,
  window-only): no link and no chat can ask the question, answer it, or answer it in advance.
- **The words are purlis's; the names are data.** A repository wrote each name, so the Notice
  and Settings draw it apart from the sentence, with every character that draws as nothing
  written out, and each is judged as any grant's target is before anything is kept. A socket
  is named as the Allow would keep it, links resolved, with the path the project wrote beside
  it where that is another, so a harmless name that leads to another socket is shown for what
  it is. A socket path that cannot be said plainly (a character that draws as nothing, or longer than a path
  is) declares nothing. A project declares at most 32 sockets and services together, so the
  question is always one a person can read whole; what a file lists past that declares nothing.
- **A chat's pane says it from the start**: a chat that starts while something declared is
  neither allowed nor declined says so in one line, and where to allow it. The line counts
  what is missing and names none of it: it is a sentence of purlis's, whole. A chat started
  after the Allow has it; one already running is listed on the "Sandbox changed" Notice with
  Restart, which now counts sockets and services among what a sandbox is made of.
- **Policy.** Under `socket-grants: false`, or `allow-scopes` without `you`, the Notice says
  who locked it and offers nothing but "Got it".
- **The certificate check is not part of this**: it is no grant any more (R1 above), so it is
  never declared, listed or asked about.
- **The onboarding steps have no step that introduces the sandbox**, so none was added there;
  the Notice appears when the project a person just made or got opens.

## Amended 2026-10-13 (operator ruling C3): the policy's word for every project

> **C3.** The policy file gets a word to forbid the every-project scope (a new public policy
> key: approved).

- **`machine`** joins `once`, `chat`, `you` and `project` in the machine policy's
  `sandbox.allow-scopes`: the scope "on this machine, for every project". It is the word the
  level is audited and listed by already.
- **A list that leaves it out removes the scope**, as a list without `you` removes "for me on
  this machine": no Notice offers it, an Allow at it is refused with the policy's sentence and
  who set it, and neither word stands in for the other. **A policy with no `allow-scopes`
  key leaves every scope**, this one included, as before.
- **The word widens nothing.** Only a local socket or a system service is ever kept for every
  project; `socket-grants: false` still takes every one away whatever the scopes say; and a
  policy file purlis refuses keeps no scope at all.

## Amended 2026-10-13: what was kept is judged against the policy at every start (#1927)

A policy that fixed `allow-scopes` stopped new grants at the scopes it left out, and what a
person had kept there before the policy arrived went on reaching every start. It does not
now.

- **Every level's kept grants are judged against the policy as it stands, at each start.** A
  folder, a socket or a service kept for this project on this machine, a socket or a service
  kept for every project, and whatever one chat holds (for the chat, or once), each
  contributes nothing while the policy leaves its scope out. The other locks hold as before.
- **Nothing is taken off the record.** What is held stays where it was kept, and Settings ›
  Sandbox › Allowed lists it as not in force, held by policy, with who set the policy and
  with Remove. `purlis doctor` counts it (`sandbox grants held`). When the policy lets the
  scope back, it is in force again from the next start.
- **Held is not allowed.** A chat blocked on something held is asked about it at the scopes
  the policy leaves open, not told to restart; and what a project expects that was allowed
  at a held scope waits again.
- **A host an Allow kept for you or for the project is held the same way** (review of the
  change; the dispatcher's delegated decision, flagged for the operator). Those lists are
  ones Settings edits as well, so this machine's record of what an Allow kept tells an
  Allow's host from a line someone wrote: only the first is held by a scope. A line someone
  wrote in a file they edit by hand stays the file's, under `personal-hosts`, `hosts` and
  `never-hosts`. The record is this machine's and this clone's: a host kept before the
  record existed, on another machine, or in a clone at another path reads as a line someone
  wrote and stays in force. `personal-hosts` and `hosts` are the locks that hold every host.
- **`once` is a word of the list like the others** (documented with this change; it was
  accepted and unwritten). A list without it removes "Allow once", which is a blocked
  socket's first answer, and a once-held grant then reaches no start. That is the list doing
  what it says. The Notice names each scope the policy removed and who set it, so an answer
  is never only missing. Flagged for the operator: whether `chat` should imply `once`.

## Amended 2026-10-13: a block's own Allow keeps the socket its Notice drew (#1927)

The up-front Allow of what a project expects keeps an item only while it still stands as the
question drew it. A block's own Allow of a local socket is held to the same rule now. Its
Notice names the socket as the kernel named it when the Notice was told, and may be answered
much later. Where that path leads somewhere else by the press, nothing is kept: the Allow
says so in purlis's own words, and the Notice is told the block again as it stands, so what
the person reads next is what an answer would keep. The rule is in the one judgement every
Allow of a socket passes, the question for several tasks and the Inbox's answer included.

## What is not done

- **Brokered Docker**, purlis running `docker` for the chat after checking the command, is a
  safer route than granting Docker's socket, and a later, optional follow-up (#1807).

## Consequences

- A socket in a folder a chat may write is never offered, so a container engine whose socket
  sits in the temp folder (Podman's default on macOS, `$TMPDIR/podman/…`) is refused with a
  sentence saying so. Use the socket it also keeps in a folder of yours that chats cannot write
  (Podman's under `~/.local/share/containers/podman/machine/`), and allow that one.
- A socket directly in a system folder (`/var/run`, root's, writable only by a system group) is
  judged as one the system keeps; a socket in a folder anyone may write (`/tmp`) is not offered.

- A grant of a container engine's socket is, by the words the Notice says, about the same as no
  sandbox for that chat. The person chooses it knowing that, as they choose Start without the
  sandbox today, but scoped to the one socket and revocable in Settings.
- The Allowed list in Settings shows socket and service grants beside hosts and folders, each
  with Remove.
