# purlis

A desktop app for running many agent chats across your projects, and always knowing which of
them needs you. This file is the glossary: the words, not how they are built.

## Language

### The five concepts

purlis has five concepts: **Project**, **Workspace**, **Chat**, **Persona** and **Memory**.
Every other word here is a part, a view or a setting of one of them, and the first hour's
screens say only those five plus **Save** and **branch** (ADR 0072).

**Project**:
One of the five concepts: the git repo that holds a team's workspaces, personas, memory and
settings, and the tab the app shows it in. The app can hold several. Vaults, save modes,
extensions and everything that holds for the whole machine are its settings. A code repo is
never a project, on any forge: it is a **repo** (ADR 0072).
_Avoid_: instance, plane (in UI text), project (for a GitLab repo)

**Project template**:
A part of **Project**: a stack's starting layout, chosen in the first run — two personas, a
starter `workspace.md` context, a review checklist (`REVIEW.md` in the reviewer persona's refs)
and the commands every harness asks about first. purlis ships one each for Rust, TypeScript,
Python, Go, monorepos and docs-only repos, as data with a version, and lays one out only where
nothing is there yet (FR-17).
_Avoid_: plane template, scaffold, starter kit

**Workspace**:
One of the five concepts: a named piece of work inside a plane, with its own charter
(`workspace.md`), memory, todos and repos.
_Avoid_: task, context

**Chat**:
One of the five concepts: one conversation with an agent, in a tab, in one workspace or at the
plane root. It is where work happens and what needs you. It has a number the window shows
(`steward 3`) and an id that never changes, has one or more **runs**, and shows its
**branches**, one per repo it works in (ADR 0066, ADR 0072).
_Avoid_: session (that is the process), thread, agent, conversation (that is the harness's)

**Persona**:
One of the five concepts: a role a chat runs as for its whole life, with its own charter
(`persona.md`), memory and vault. A chat's persona is fixed when the chat starts and never
changes, and work for another persona goes to a chat of its own, by **dispatch**. Its curation
actions and its logs are parts of it (ADR 0072 as amended, ADR 0090).
_Avoid_: agent, sub-agent (that is a harness's helper inside a chat, which carries the chat's
persona and is never one itself), bot, role (as the name)

**Memory**:
One of the five concepts: what purlis keeps so the next chat starts knowing what earlier ones
learned. Each memory has an **owner**: a workspace, a persona, everyone (shared memory, in
`personas/_shared/memory/`) or **me**, one person's own, kept in a personal overlay plane on
their own private remote. It also has an **audience**: this machine, me on all my machines, or
the team. Approval follows the audience: memory the team will read waits for approval, so
nothing reaches a teammate's briefing unreviewed. Session records are memory too. `purlis recall`
searches it as one, and a chat's briefing is drawn from it (ADR 0072).
_Avoid_: knowledge, rules, notes (for the whole of it), context

### The plane and what lives in it

**Plane**:
The git repo a project's purlis lives in: its settings, personas, memory, todos and
workspaces. It is the project's database, and a change counts once it reaches the plane's
remote. **The word is being retired**, everywhere: the new term is **Project**, in the window,
`purlis --help`, the docs, the code and the format. Until the rename lands, the code and the
format still say plane, and purlis reads the old names for a compat window (ADR 0072).
_Avoid_: control plane, config repo, purlis repo

**purlis-plane**:
The purlis project's own plane: the one purlis is developed from, at `purlis/purlis-plane`,
private since it moved there (V92). It is not the product. The product, the app and its core, is
**purlis**.
_Avoid_: purlis (for the plane), the purlis repo

**charter** (the old name):
What purlis was called until 2026-10-05 (ADR 0091). purlis still reads its old names until 1.0
(the compatibility window), the `charter` command runs purlis until then, and names the code
still spells `charter` keep that spelling. In new prose it is only ever the old name, or a role's
charter: the English word for what a persona or workspace is for.
_Avoid_: charter (for the product)

**Work item**:
One piece of work a tracker holds: a forge issue, epic or sub-issue, or a todo (the plane's own
tracker). A chat links to at most one; a work item may have many chats. It is part of a
Workspace, shown in its Work section, and never a sixth concept (ADR 0072).
_Avoid_: task, ticket (in UI text), card (that is how a board draws one)

**Tracker key**:
A work item's identity: the tracker's name and the item's own reference where it lives, such as
`github:github.com/owner/repo#12` or `todo:<workspace>/<todo>`. Two devices name the same item
the same way. When an item moves, its old key is kept and points to the new one. It belongs to
**Workspace**, with the work item it names (ADR 0088).
_Avoid_: issue id, forge id (that is the forge's own identifier, which travels beside the key)

**Work link**:
A workspace's or a chat's record that it works on a work item. A chat has at most one; a
workspace may have many. The project keeps them, so they reach every device. It belongs to
**Workspace** (ADR 0088).
_Avoid_: link (unqualified: that is the runner link), relation

**Relation** (between work items):
How one work item stands to another: a pull or merge request that closes it, blocked-by, or
parent and child. It belongs to **Workspace**, with the work items it joins (ADR 0088).
_Avoid_: link (unqualified), dependency (that is one kind of relation)

**Board** (of work items):
A view of a workspace's (or project's) work items, one card per item. It is derived each time
from the work links and the trackers, and never stored, so a todo promoted to an issue is one
card. It belongs to **Workspace**, in its Work section (FI5).
_Avoid_: project (GitHub's word for its boards), kanban

**Plane root**:
The plane's own directory, as a place a chat works — and anywhere else in the plane that is no
workspace's, such as `docs/`: the workspace strip's first tab, drawn as an icon, always there. A chat started there is in no workspace on purpose — it looks after the
plane and names a workspace with `-w` when it acts on one. It is not a workspace: it has no
purlis, memory or todos. In code it is still `OUTSIDE`, the strip every chat working in no
workspace is filed on.
_Avoid_: master, home, outside every workspace (in UI text), the default workspace

**Repo** (of a workspace):
A clone of a code repository that a workspace holds. It has its own remote and its own rules,
and it is never part of the plane's commits.
_Avoid_: clone (as a noun in UI text), guest checkout, project

**Piece**:
A git worktree of a workspace's repo, at `workspaces/<ws>/.worktrees/<repo>/<piece>`, where
one chat works on its own branch. Git says which pieces exist. The piece log says what git
cannot: that purlis cut it (`claimed`), and whether its worker declared it `done` or
`abandoned`. A piece that declared nothing is **silent**, reported as an age and never as a
failure.
On screen a piece is shown as its **branch**, and its directory as the branch's **folder**:
the explorer's rows, the palette's titles, the Changes view and the refusals the window shows say
"branch" or "folder", never "worktree" (#989). A menu row names a branch by its own name, and
a folder git has on no branch as a folder. "Piece" and "worktree" stay in the plane format, the
code, the `[plane] worktrees` setting and `purlis worktree` (ADR 0072).
_Avoid_: task, slot, branch (for the directory), worktree (on screen)

**Branch** (of a chat):
What the window shows for a piece: one row per repo a chat works in, reading *`<branch>` in
`<repo>`*. A chat working in a repo's shared clone shows the branch the clone has checked out,
marked *shared*. **New branch** cuts a piece; **Remove folder** removes the worktree and keeps
the branch (ADR 0072).
_Avoid_: the chat's branch (singular: a chat may work in several repos), piece, worktree (in UI text)

**Writing chat**:
A chat that starts in a repo's shared clone (`workspaces/<ws>/<repo>`), which is what *New tab
in <repo>* and *Start new chats in <repo>* point at. By default it starts on a new branch of its
own instead: a piece named after the chat, or `chat-<n>`, cut before it starts and taken back if
the start is refused. The picker's *start on a new branch* box, ticked by default, is the
opt-out. A chat started in the workspace's own directory, at the project root or in an existing
piece is not a writing chat and is cut nothing (GL-1).
_Avoid_: guessing from what a chat does (purlis never reads a harness's output to decide)

**Change** (cross-repo):
One piece of work across several of a workspace's repos, recorded as intent only in
`workspaces/<ws>/changes/<slug>.json`: why, which repos, which branch in each, and which must
land first. Whether each part is pushed, checked or landed is read from git and the forge, and
never stored (ADR 0060).
_Avoid_: change (for one pull request), changeset, epic

**Member**:
One repo's part of a change: the repo, its branch for this change, and the members it `needs`
landed first.
_Avoid_: part, sub-change

**Request**:
A member's pull request, or merge request on GitLab.
_Avoid_: change (for a PR), MR/PR in UI text

**Landed**:
A member whose request the forge reports merged, that purlis landed, and whose merge commit, as
purlis's landing log recorded it, is still on the default branch. A merge purlis queued, or
whose read-back failed, is landed once a later `purlis change land` finds it merged at its
**pending landing**'s head and logs it.
_Avoid_: merged (a browser merge is merged but not logged), done

**Pending landing**:
purlis's evidence that it started landing a member: the request, the head its checks passed on
and how (direct or queue), written before the forge is asked. Only a merge at that head is ever
logged as purlis's; a refused one is no evidence.
_Avoid_: queued (a pending landing is also written for a direct merge), in flight

**Inventory**:
The plane's list of repos it can clone (`inventory/repos.json`), committed and shared. It only
grows: `discover` and the repo picker add to it, and a repo leaves it only through an exclude.
_Avoid_: repo list (for what the picker shows), catalogue

**Reachable repos**:
The repos your own forge login can reach under the plane's owners, as the repo picker shows
them. Asked each time the picker opens and never saved, because each engineer reaches different
ones.
_Avoid_: discovered repos, the inventory

**Forge account**:
One sign-in to one forge host: a kind, a host and a login, held in the keyring or reached
through `gh`'s or `glab`'s own login. Each repo is bound to one (ADR 0070). It is the human's,
signed in from the window through a forge registration, a PAT or an imported CLI login, and it
never reaches a chat. It holds for the whole machine, so it is a setting of the **Project**
(ADR 0072 §2, ADR 0077).
_Avoid_: forge login (for purlis's own sign-in), connection, integration

**Forge registration**:
What a forge host knows purlis by when a person signs in. On GitHub it is an OAuth app with the
device flow on; on GitLab it is an OAuth application that is not confidential and signs in with
PKCE. Either is identified by a public client id and holds no secret. purlis's own are
registered under the purlis organisation on github.com and the purlis group on gitlab.com. A
GHES or a self-managed GitLab needs one made on that host, and until then it signs in with a
token. It mints nothing for an agent. It is a setting of the **Project**, like the forge account
that uses it (ADR 0072 §2, ADR 0077 as amended 2026-10-12).
_Avoid_: OAuth app (unqualified), GitHub App (for purlis's sign-in), integration, client

**Forge capability**:
One thing a forge may or may not do for one repo, such as a merge queue, judged per forge, host
and tier, with the fallback purlis uses where it is unavailable (ADR 0070).
_Avoid_: capability (unqualified, which is an extension's), feature flag

**LIVE / LOCAL**:
Whether a workspace's charter, memory and todos are published with the plane (LIVE) or stay on
this machine (LOCAL, the default).
_Avoid_: shared/private, public

**Tier** (of a store):
Where a file purlis keeps lives, and so what a backup, a second machine and a deletion do to
it. **Plane** is committed. **Clone state** is per clone and never committed: `.charter/`,
`charter.local.toml` and a LOCAL workspace's files. **Machine** is outside every plane, and each
store there is syncable or device-bound. **Keyring** is the operating system's credential store.
A derived store is also marked rebuildable (ADR 0069).
_Avoid_: app data (for the Machine tier as a whole), cache (for clone state), local state

**Clone-key**:
A project clone's identity on a machine: the first 16 hex characters of the SHA-256 of the
clone's canonical root path. It keys the reopen record's `clone` (V43), the search index will
be keyed on it (ADR 0079), and a save branch's default name takes its first six. A clone's own
word for itself, never committed, and different in every copy at another path. A copy at the
same path on another machine has the same key, and the device id is what tells it apart.
_Avoid_: clone id, plane hash, install id

**Place** (of a project, in the session host):
A project's stable id at one clone's canonical root on the host's machine: what the host holds,
deals chat numbers in, hears hooks from and saves. Two clones of one project are two places;
one folder is one place however its path is spelled. A link holds each project at one place
(ADR 0068, amended for #2178).
_Avoid_: path (for the place as a whole), clone-key (that is the clone's own word for itself)

**Copy** (of a project):
A second clone made from a project's directory, `.charter/` and all: `cp -R`, rsync, a backup
restored beside the original, or the same path on another machine. Its first launch finds that
another clone still holds its chats, or that another device wrote its record, and gives every
chat a new id (V43). A `git clone` is not a copy in this sense: it carries no `.charter/`.
_Avoid_: fork (that is a workspace's), duplicate

**Move** (of a project):
The same clone at a new path on the same machine. Its first launch finds no other clone holding
its chats, so the chats keep their ids and the record names the new clone-key (V43).
_Avoid_: rename (that is a workspace's), relocation

### Runs and devices

**Run**:
One stretch of a chat's conversation with a harness, over which its persona, harness, profile,
model source, device, sandbox and harness level stay the same. A chat has one or more runs, one
after another. A new one begins when the chat starts, on `/clear`, when the app reopens it, when
it wakes, when it starts again without its conversation, or when any of those attributes
changes. Compaction keeps the run. A **child run** is a sub-agent or teammate the harness
spawns, with the run it came from as its parent: a **helper**, as the app shows it. The run is
who an action is attributed to (W8's "agent run"), and budgets add up over a chat's runs (ADRs
0066, 0073).
_Avoid_: session (that is the process), conversation (that is the harness's), turn

**Run state**:
Where a run is. While it lives: `queued`, `starting`, `working`, `input-required`, `paused` or
`hibernated`. Then, once and for good: `completed`, `failed` or `stopped`. **Stopped** is an end
someone chose (the operator, a policy, the kill switch, the host); **failed** is one nobody chose.
A chat's state is its current run's (ADR 0076).
_Avoid_: status, session state (the old five), done (for completed), idle (for a state)

**Hot chat**:
A chat whose current run has a process: `starting`, `working`, `input-required` or `paused`.
An **open** chat is any chat whose current run is live, hot or not; a hibernated one is open and
not hot. purlis's scale is counted in these per device: a target of 200 open, and a hot target
per **RAM class** (ADR 0082).
_Avoid_: active chat, live chat (live is a run state's), running chat (in UI text)

**RAM class**:
A device's physical memory, as the largest of 8, 16, 32 and 64 GB it reaches, which sets how
many hot chats the device targets; a device under 8 GB targets one. A property of the device, so
it belongs to Project. A budget, never a cap: purlis warns past it and refuses nothing (ADR 0082).
_Avoid_: tier (that is a store's), machine size, profile

**Performance budget**:
One measured limit on what purlis's chats cost the device: a count, a size, a time or a share of
a core, stated at the device's hot target, and named with the job that measures it. It is
checked in CI either exactly or against what `main` last recorded, or on the operator's machine
at each release. It is a target, never a promise, and never a cap on the operator. The spec's
speed and memory limits are performance budgets too; its open and hot chat counts are targets. It belongs to **Chat** (ADR 0086).
_Avoid_: budget (unqualified: that is a chat's spend budget), limit (for a row CI checks only
against earlier runs), SLA

**Remote chat**:
A vendor-cloud session purlis lists read-only, with its state, pull request and cost: a chat of
kind **observed**, which purlis never pauses, stops or counts toward a budget. Every chat
purlis starts is **governed** (W8, ADR 0076).
_Avoid_: cloud chat, external agent, remote runner (that is a **Runner**)

**Device**:
A machine purlis runs on: a desktop, a runner, or later a viewer. Each has a random id kept in
its machine store, which is how records, events and the audit say where something happened. Its
hostname is a label, never a key. The operator on a device is its **local principal**
(`local:<device>/<os-user>`), and purlis never sends it anywhere without an account (ADR 0066).
_Avoid_: host (that is `purlisd`, the process), machine (in UI text), node

**Harness declaration**:
Data that says how to start one harness, how to name and resume its sessions, which levels it
offers, what it can do, and what its profiles are: the **sign-in kinds** they support, the
folder variables, whether their credentials can be kept from the harness's commands, and how a
conversation moves. The ones for Claude Code, Codex and opencode ship with purlis; a project may
declare more, which each machine approves before they run, and never replaces a built-in's
(ADR 0073).
_Avoid_: harness definition, harness config (that is the harness's own), profile (that is one
identity of the harness the declaration describes)

**Harness level**:
How much purlis learns from a chat's harness, set when a run starts and fixed for it: **1**,
the terminal alone; **2**, the terminal with the harness's own hooks reporting to purlis; **3**,
a structured protocol, ACP or the harness's own. A fall back to a lower level starts a new run.
Never shown on a first-hour surface (ADR 0073).
_Avoid_: tier (that is a store's), mode, integration level

**Harness capability**:
One thing a harness does or does not do for a chat, such as report that it is waiting: yes, no
with the fallback purlis uses, or unknown, which reads as no. The capability card shows the
*no*s in plain words (ADR 0073).
_Avoid_: capability (unqualified, which is an extension's), feature, support

**Model choice**:
The **profile** a chat runs on and a model: the profile says how it signs in and who is billed,
and the model, until one can be chosen, is the harness's default. Set on the chat, or given as a
default by its persona, its workspace or the project, and fixed for a run. It is always shown
with what it is billed to (ADR 0087, amended by #2036).
_Avoid_: model config, model settings

**Model source**:
How a chat reaches its model: its **profile**'s sign-in kind. A `subscription` signed in through
the harness's own flow, an `api-key` or `cloud` credentials the person keeps in a vault and the
profile names by reference, or a `local` model. None of them passes through purlis's servers. A
subscription's sign-in is the harness's, kept in the profile's folder, and purlis never reads,
copies, stores or relays it (ADR 0087, ADR 0095).
_Avoid_: provider (that is who serves the model), backend, BYO (for the whole of it)

**Profile**:
A harness as the person runs it on a device, under a name: the harness, its settings, its
sign-in, its conversations and the browser logins kept under it. It is the person's: a persona
names one, a chat runs on one, and it lives on its device and is never committed. A
subscription is signed in once per device through the harness's own flow, and purlis never
holds it. Every sign-in a chat uses is a profile's (ADR 0095). One browser writes a profile's
kept logins at a time, and a second chat that wants them is offered a copy. A profile purlis
makes **for sandboxed chats only** runs no chat outside the sandbox, and a sandboxed Codex chat
runs on no other (#2037).
_Avoid_: browser profile, kept profile (both are the profile), harness folder (that is where a
profile is kept), harness login, login (for its sign-in: a login is a browser's, or a **browser
login**), account (that is who a model is billed to), credentials (that is what a
sign-in leaves in its folder), user data, persistent browser

**Sign-in kind**:
How a **Profile** signs in to its model: a **subscription** through the harness's own flow,
which the harness keeps and purlis never holds; an **API key** or **cloud** credentials from
the person's vault; or a **local** model, with nothing to sign in. A harness declaration lists
the kinds it supports (ADR 0073). Not a forge's sign-in, which is a **Forge account** and never
reaches a chat.
_Avoid_: login (unqualified), auth method, account (that is who a model is billed to)

**Session host** (`purlisd`):
The process that owns every **hosted program** on a device, every chat's terminal and every
browser, one per OS user per device. On a desktop the window starts it with its own binary and
is its first client; on a machine with no window it is `purlis serve` (ADR 0068, amended for
#1997).
_Avoid_: daemon, server (in UI text), backend

**Hosted program**:
A program a session host runs and owns: a chat's harness, a shell, or a browser. Each has an
owner and a process of its own, may be pinned, outlives the client that watches it, and is
watched through **views**. Starting, attaching, stopping, reattaching and the kill switch work
the same for every kind (ADR 0068, amended for #1994).
_Avoid_: process (that is the OS's), session (that is the host), job, task (that is a
dispatch's)

**Kept board** (of a held project):
The session host's copy of what each chat of a project is doing and its **Needs you** items,
kept so it outlives the window. While a window holds the project, the window decides each move,
makes it on its own copy and tells the host, which makes the same move; with no window there, the
host moves it by the lines the chats' hooks write. A window that holds the project takes it whole
(ADR 0068, amended for #2194). Not the **Board** of work items, which is derived and never stored.
_Avoid_: board (unqualified: that is the work items'), state store, snapshot

**Host record**:
The session host's own note of the hosted programs it has started and not yet ended, kept on
the device so that, when a host is killed outright, the next one ends what it left running and
keeps each chat's end as a host crash's (ADR 0068 §3, amended for #2007). Never read by the
window. Not the run journal (ADR 0076), which holds each run's state.
_Avoid_: run journal (that is ADR 0076's), host log, journal (unqualified)

**Grace period** (of the session host):
How long the window's session host keeps a window's chats after the window went without a quit
(it crashed, or was killed): ten minutes. A window that starts within it **reattaches** every
chat, on the screen it has now; a project no window holds again is let go of once its own grace
period passes, its chats ended as a quit would, and the next start offers them for reopening
(ADR 0068 §2, amended for #2006). A quit has none.
_Avoid_: timeout, linger, keep-alive

**Reattach** (a chat):
What a window that comes back does with a chat the session host kept: it takes the chat back as
one of its own, under its number, its conversation and its run, without starting it again. Not
a reopen, which starts a chat's program again on its conversation.
_Avoid_: resume (that is the harness's), restore, reconnect (that is the link's)

**View** (of a hosted program):
One client's live look at a hosted program, of a kind: a terminal view, or a **browser view**.
It opens with a snapshot of the program as it is now and then follows it. One that falls behind
is dropped and opened again from a fresh snapshot, and a client that goes away loses its views
at once while the programs run on (ADR 0068, amended for #1994).
_Avoid_: stream (that is the link's), pane (that is the window's), view tab (that is a tab of the
window), subscription (that is events), viewer

**Session protocol**:
The small, public, versioned set of commands and events a session host answers on its control
lane: list, start a hosted program, attach and detach a view of a kind, write, resize, answer,
stop, and subscribe from a cursor.
It carries the compatibility promise, so a client and a host one version apart always talk.
It names a project by its stable id (`[project] id` in `charter.toml`), never by a path, and
every word it says is snake_case (ADR 0068 §4 and *Amended by FD-26*).
_Avoid_: API (unqualified), IPC (that is the window's Tauri calls)

**UI RPC**:
The app's own commands (`ipc_commands.rs`), called by the window on the same control lane as
the session protocol. Private to one build of the app, served only to `local-ui`, and promising
nothing across versions (ADR 0068 §4, FD-26).
_Avoid_: session protocol (for any of it), public API

**Runner**:
A device, other than the one the window is on, whose own session host runs a workspace's chats,
serves a version of one of its repos, or runs browsers, as its **label** says; one marked owned
may run chats and browsers in one session host. A long-lived runner
is a machine the operator adds; a short-lived one is made by a **runner provider** for a human or
a chat, for minutes or hours. Either way the desktop's session host reaches it through a
**connector** and talks to it over a **link**, and its shell, logs, ports and browser views come
only that way. It keeps its own device id, event log, audit chain and kill switch, and it needs
no server purlis runs. A short-lived runner is owned by the chat that started it, or by its
workspace when a human started it or **pinned** it there. A runner is a device, so it belongs to
**Project**; which runner a workspace's chats run on is a **Workspace** setting, and the
**Runners** view tab shows a workspace's runners (ADR 0072 §2, ADR 0078, ADR 0089).
_Avoid_: remote (unqualified), agent host, worker, server (in UI text), sandbox, environment, box,
belongs to (for the lifecycle sense: say "is owned by")

**Connector**:
The command whose stdin and stdout reach a runner's session host: `ssh <alias>`,
`gh codespace ssh`, `coder ssh`, `docker exec -i`, `kubectl exec -i`, and later the relay. Held
as an argument vector on the machine that uses it, never committed in a project. It gives
reachability, never identity. It belongs to **Project**, with the device it reaches (ADR 0072
§2, ADR 0078).
_Avoid_: transport (that is FD-4's framing), tunnel, provider (that is the **runner provider**,
which creates the machine)

**Bridge**:
`purlis bridge`, the program a connector runs on a runner: a pipe from its standard input and
output to that runner's session host, which ends the link's handshake itself. Its one other act
is a runner's pairing, once, with `--pair`; on a cluster runner, also making the runner's link
key as its pod starts, with `--make-key`, which its pairing then takes. It belongs to
**Project**, with the connector it serves (ADR 0078 §2, as amended 2026-10-17 and 2026-10-26).
_Avoid_: agent, relay (that is the desktop host's relay, or later the hosted relay), proxy

**Link**:
The encrypted, mutually authenticated stream between the desktop's session host and a runner's,
carried by a connector. Each end proves itself with its device's link key, pinned when the runner
was added, in a Noise `XX` handshake. The desktop always opens it, keeps it alive with the
link's own ping, and links again on its own when it drops. Its protocol has a version of its
own, apart from the session protocol's. It belongs to **Project**, with the two devices it joins
(ADR 0072 §2, ADR 0078).
_Avoid_: connection (unqualified), session (that is the process), pairing (that is how the keys
were pinned)

**Label** (of a runner):
What a runner not marked owned is for: exactly one of `harness` (runs chats), `app` (serves one
version of a workspace repo) or `browser` (runs browsers only purlis drives). purlis enforces it
at both ends of the link: only a `harness` runner starts chats. A runner not marked owned keeps
its one label and never receives a vault value; a device the person owns may run chats and
browsers in one session host, and receives a value only with a **grant**. Tags, any words the
operator picks, sit beside it and are enforced by nothing. A part of **Runner**, so it belongs
to **Project** (ADR 0089, amended 2026-10-14).
_Avoid_: role (unqualified), type, capability (that is an extension's), tag (for the role label)

**Runner provider**:
What makes and destroys short-lived runners and hands back a connector to each: purlis's own
core provider, which drives Docker or Podman on this machine, or an extension on the provider
seam for anything else. It declares which labels it may make; whether its machines are the
user's own is the operator's setting for it, never its own claim, and starts as not owned. A
part of **Project**, beside the extensions it is one of (ADR 0078 §6, ADR 0089).
_Avoid_: provider (unqualified: a vault's provider is where a vault keeps its values, ADR 0047,
and a model's is who serves the model, ADR 0087), backend (in UI text), executor, cloud,
connector (that only reaches a runner), pool (that is what the person picks; a runner provider
is the code behind one)

**Pool**:
A named place runners come from, with its limits and what its runners may hold: Docker on this
machine, or a cluster that makes them on demand. A cluster's is defined once for a team. A
**runner provider** is the code behind one. It belongs to **Project**.
_Avoid_: node pool (that is the cluster's), fleet, cluster (for the pool), warm pool (that is one
setting of it), runner provider (that is the code)

**Claim**:
One person's request for a runner from a pool, and the runner it is given. A **kept** claim has
storage of its own and survives its runner; a **fresh** one is given a ready runner and keeps
nothing.
_Avoid_: claimed (of a piece: that is the piece log's word), lease, reservation, sandbox

**Cluster operator**:
purlis's controller inside a cluster: it turns claims into runners, counts them per person,
starts and stops pools, and says which runner a claim was given, vouching with its own signing
key for the link key each runner made as it started (ADR 0078, amended 2026-10-26). In a
sentence the app shows,
"purlis in your cluster".
_Avoid_: operator (unqualified, where the person could be meant), controller, manager, master

**Grant** (to a runner):
A human's approval, once per workspace on this machine, of one vault entry an app runner's spec
declares it needs. purlis injects the value when that runner starts and only when it builds a
ref a human chose, never an agent's own unreviewed branch, and never into its image or logs. An
agent may start a runner with the grants it has and never adds one; a runner not marked owned
never gets one. A setting of **Workspace** (ADR 0089).
_Avoid_: permission, secret (for the approval), capability (that is an extension's)

**Pool login opt-in**:
The person's answer, in an Ask, that one persona's browser login may be filled in its chats'
browsers on one pool's runner: bound to that login as declared and allowed, to the pool as it is
defined, its image, the runner that receives the values and the engine it runs on, and kept only
while purlis runs. It is asked only where purlis can show the runner is the person's own: until a
pool can be marked owned, a Docker pool on this machine's own engine. A runner not shown to be
owned is never asked about and never receives a value. A login that waited while the desktop
was away is asked about afresh. It adds to ownership, never stands in for it (ADR 0089, amended
2026-10-26).
_Avoid_: grant (that is an app runner's vault entry), owned (that is the device), Always (it lasts
one run)

**Editor protocol**:
The part of the session host's public protocol an editor integration speaks: find which chat or
workspace a file belongs to, ask the window to show one, type a selection into a chat's prompt
without sending it, and list a chat's changed files. Nothing in it starts, sends to or answers a
chat. It is a part of the session host, so it belongs to **Project** (ADR 0072 §2, ADR 0081).
_Avoid_: editor API, editor extension API (that is a non-goal: code running inside purlis's editor)

**Audit**:
The record of who did what, for whom, to what, and whether it was allowed, kept per device by
the session host as **audit entries** in purlis's data home, never in a project. Once AU-3
lands, the entries are a device-signed hash chain. It is never sampled, and it is a separate
system from telemetry (ADR 0075).
_Avoid_: log (unqualified), history, `purlis secret audit` (that is a vault health report)

**Audit entry**:
One line of the audit, written from one event: an action, its actor and whose behalf it acted
on (both as keyed pseudonyms), what it acted on, its outcome, and typed metadata. Never a prompt,
output, file contents, raw arguments or a secret value. An agent's entry says what purlis could
see of its run: its **coverage** (ADR 0075).
_Avoid_: audit event (the event is what the entry is written from), log line

**Coverage** (of an audit entry):
What purlis could see of the run an agent's audit entry is about, set by its harness level:
the process only (level 1), the tool calls its hooks report (level 2), or every tool call its
protocol reports (level 3). A level-2 run whose hooks never reported is **unarmed**, and a
vendor-cloud chat purlis only lists is **observed**. Shown beside the entry, so silence is never
read as "did nothing" (ADR 0073, ADR 0075).
_Avoid_: level (that is the run's), completeness

**Telemetry**:
What purlis measures about how chats and purlis itself perform: time, resources, tokens and
cost. It is one OpenTelemetry pipeline per device: the session host receives what the harnesses
and purlis report, keeps it on the device for a month, and sends it only to the user's own
**export destinations**. The opt-in product telemetry and crash reports are telemetry too, each
with its own consent. It may be sampled, never names a person, and never reads the audit (ADR
0075, ADR 0083).
_Avoid_: audit, analytics, metrics (for the whole of it), log (that is the diagnostic log)

**Content gate**:
A switch that lets telemetry keep what was said in a chat: `prompts`, or tool arguments and
results (`tools`). Both are off by default. Only the human opens one, for a project, on this
machine; a project's settings or an org's policy can only close it (ADR 0083).
_Avoid_: redaction (that is removing secrets from text), privacy mode

**Export destination**:
A backend the user added, such as Langfuse, Grafana or Datadog, that the device's telemetry is
sent to over OTLP. A machine setting, never a project's, and later also an org's (ADR 0083).
_Avoid_: exporter (that is the harness's own sender), sink (that is the audit's)

**Search index**:
A derived SQLite full-text index that search, `recall` and the briefing read, kept so they need
not scan every file. Each project clone has one over its memory, session records and todos, in
`.charter/`; each machine has one over the transcript archive, in purlis's data home. The
session host is its only writer, and a chat searches only its own clone's. It is never the
truth: deleting it costs a rebuild. It holds the words of what it indexes, with their positions
and after redaction, but not the text as written. A deleted item stops answering at once and
leaves the file at the next merge and checkpoint; an unlinked old copy is not overwritten. It
never answers a reader with anything the files would not (ADR 0079).
_Avoid_: index (unqualified: `memory/index.md` and `sessions/index.md` are lists for people),
cache, database

### Browsers

**Browser**:
One isolated world of cookies, storage and logins that a session host runs as a **hosted
program** for a chat, a task or the person, and that only purlis drives. It starts **fresh**,
keeping nothing, or from the logins kept in a **profile**. A chat has one by default and may ask
for another; a task gets its own. It is watched in a **browser view** and never as a window of
its own. It belongs to **Chat**, or to **Workspace** once pinned.
_Avoid_: browser session (session is the host), instance, window, context, headless browser,
browser profile (that is the **profile**)

**Page**:
One of a browser's open documents. Pages of one browser share its cookies and logins.
_Avoid_: tab (that is the window's), frame, document

**Snapshot** (of a page):
The agent's reading of a **page**: its elements as a tree of lines, each with its **reference**,
its frames named and placed under the element that holds them. It is read in parts, from a line,
or within one element, and a find answers the lines that match. Not a picture of the page (that
is a screenshot), and not a **view**'s snapshot (a **browser view**'s is its latest frame).
_Avoid_: DOM, page source, accessibility tree (that is what it is made from)

**Reference** (to an element):
The name a **snapshot** gives one element of a page, such as `e12`, by which the agent acts on
it. It names the same element in every snapshot of that document, and dies when its frame loads
another document.
_Avoid_: selector, handle, node id, ref (in prose)

**Blanked**:
Said of an element of a page whose content no answer to the agent shows: the field purlis filled
for a **browser login**, and every field the person typed into. A snapshot shows it by its role
and reference only, a screenshot has it painted over, and what it held is kept out of every answer
about its page for as long as the page is open. Blanked boxes next to each other, such as a code
typed one character a box, are kept out joined too. Once a page keeps anything out, a screenshot also
paints over that text wherever else a page draws it, and on pages of the same site every canvas,
image and video, which it cannot read; when what the page draws cannot be read, the screenshot
is withheld.
_Avoid_: masked, redacted, hidden (a page hides elements of its own)

**Engine**:
The program a browser runs on, one per browser: a pinned Chrome for Testing that purlis
downloads on first use, checks against its manifest before it runs, and keeps current apart from
purlis's own releases, or the machine's own Chrome when the person chooses it (the Notice offers it where the pinned engine cannot make its sandbox, #2835). It always runs with its sandbox on. The
session host drives it only over a pipe it holds, never a port, and it ends with its host.
_Avoid_: Chromium (unless that project is meant), headless Chrome, driver (that is who acts on a
browser)

**Driver**:
Who may act on a browser now: the chat's agent, the person, or purlis itself while it performs a
login. Exactly one at a time. While the person drives, the agent's browser tools are refused in
words, and the browser returns to the agent only when the person says so.
_Avoid_: controller, owner (that is whose the browser is), focus

**Driver lease**:
The session host's record of a browser's **driver**, with an epoch that changes each time the
driver does. Every act of the agent's is carried out under the lease it was checked against, so
one the person's take-over overtook never reaches the page. It stays with the person whatever
becomes of the window, until they hand it back. Only ever "driver lease": a bare "lease" is a
**claim**'s word to avoid.
_Avoid_: lock, token, session

**Take over**:
The person's act of becoming a browser's driver: a click or a key in its view. **Hand back** ends
it: Done, or one fixed chord, and nothing else.
_Avoid_: control, grab, interrupt

**The browser needs you**:
An **Ask** whose source is a chat's browser: the agent, through `ask_operator` pointed at that
browser, asks the person to do what a page needs of a human (a CAPTCHA, a second factor, a
consent page). Its one answer, Take over, opens the browser's view already taken over; the
person's **hand back** ends it and wakes the chat with "the person finished", never what they
did. It ends with its browser too, leaving an **Update** (#2015).
_Avoid_: help request, intervention

**Aim** (of the person's input):
What a **browser view**'s frame showed the person: the page, and the documents in it, as a number
that changes each time the page or any frame inside it loads another document. Each click or key
names the aim of the frame it was aimed at, and the browser drops one whose page has since moved
on, in any of its frames, rather than let it land on something else.
_Avoid_: target (that is the engine's word for a page), focus

**Pin** (of a browser):
A person's act that gives a chat's browser to its workspace: it outlives the chat but not the
app. It ends by a person, the kill switch, or the app quitting, and reopening it is offered at
the next start (ADR 0068 §2). A pinned runner, unlike it, stops only by a person or the kill
switch. Not a tab's pin.
_Avoid_: keep, hold, save (that is the plane's)

**Browser view**:
A browser's **view**: its frames as they come, drawn in a view tab, a tile of the Browsers grid
or a chat tab's thumbnail. Its snapshot is the latest frame. The same on this device and on a
runner.
_Avoid_: screen share, preview (that is a tab's), remote desktop, video

**Browser tools**:
The typed tools a chat's agent drives its browsers with, the same on every harness: a
**snapshot**, actions on its **references**, and a screenshot. None runs script. A chat's first
one asks the person what its browser may reach: this project's sites and its dev servers, also
the open web, or off; for this chat, or **Always for this persona**.
_Avoid_: browser automation, Playwright, DevTools (that is the **engine**'s)

**Browser timeline**:
A chat's record of what its agent did in its browsers, oldest to newest, with the screenshots its
**browser tools** returned, and where the person or purlis drove instead, which has no screenshot.
Kept on this device, bounded, and removed when the chat closes; it holds nothing the agent was
not given. Proof of the work.
_Avoid_: recording, history, log (that is the event log's), video

**Declared browser** (of a persona):
What a persona says its chats' browser usually reaches, in its sandbox section
(`browser = "project"` or `"open-web"`). It only marks an answer of the browser's Ask, in its place;
the person still answers it (ADR 0073: declaring never grants).
_Avoid_: browser grant, browser permission (it grants nothing)

**Always for this persona**:
An answer to a chat's browser Ask that holds for every chat opened as that persona, in this
project on this machine: kept where no chat writes, listed and removable in Settings, and
forbidden where an administrator's policy leaves out the scope it is kept at. Removing it ends
the browsers it opened.
_Avoid_: remember, persona grant (that is a persona's hosts and tools)

**Browser login**:
A persona's declared tool that signs a browser in from a vault: the chat names it, and purlis,
as the browser's **driver**, goes to the declared address, fills on its origin alone and
submits. The value reaches the browser and never the chat, and the browser then reaches that
login's site alone.
_Avoid_: autofill, credential tool, password tool

**Reach** (of a browser):
The sites and local ports a browser may open and the ones its agent may write to: its chat's
**allowed hosts**, in a lane of their own for the browser, and its **browser sites**, all shown
in the one Granted list. A site new to the project that the agent itself writes an address for,
posts a form to, or types on, asks once; a link the page offered does not, and on the open web
nothing asks. A private address is reached only by the literal address and port the person
allowed.
_Avoid_: allowlist (in UI text), network access, browser allowlist

**Browser site**:
A person's yes to one site, a host and port, for a chat's browser alone, at *this chat*, *this
project on this machine*, or *everyone in the project*, which is committed and reaches a
teammate's browsers once they were shown it: the browser may open it, send to it and type on it. The chat's own
commands never reach a browser site; they still need an **Allowed host** for it. Asked by the
browser's **judge**, and listed apart from allowed hosts in the Granted list and in the chat's
Network view. An Allow reaches the browser from its next act; nothing restarts.
_Avoid_: host grant, browser host, allowed host (that is the commands' own)

**Judge** (of a browser):
The part of a chat's browser that hears every request its pages are about to send, and every
field the agent is about to type into, and says whether it goes or the person is asked: a read
goes, an address the agent wrote or anything sent to a site new to the project asks. The reach
proxy carries a site new to the project only where the judge let a request to it go.
_Avoid_: filter, firewall, interceptor

### Chats working together

Every agent another agent starts is a chat, so each word here is a part of **Chat** (ADR 0090).

**Dispatch**:
One chat starting another, with a brief. Its mode is a **task** or a **handoff**. The chat it
starts is a **persona chat**, with its own persona, sandbox, vaults and asks. Nothing the
**asking chat** was allowed travels with the brief, and the brief is a request from a chat, never
the person's word. The person can dispatch too.
_Avoid_: spawn, delegate, delegation, sub-agent (that is the harness's child run)

**Task** (of a dispatch):
A dispatch that expects a **report**, and the chat it starts: work done for the asking chat. It
is listed under the asking chat, never hidden, and is shown inside its session's tab: the tab
of the nearest chat above it that has one. Pressing the task switches that tab to it and adds
no tab; the pane's top line then says the path (`steward 4 › talk · working`). It has a pane
of its own only when the person asks: **a tab of its own**, drawn with the task mark, whose
task it is and a minimise where a session's tab has its close, or a pane **beside** its
session, inside the session's tab. The minimise **sends it back** and ends nothing; a session's
close is the only close on the strip. "Task" is the
word in every sentence the app shows for that chat. Its row says its state in a word
beside a mark with a shape of its own, the same in every list: working, needs you,
`asking <chat>`, done, failed, cancelled, or ended without a report. A chat at rest that is
not asking for the person reads idle, and one whose harness sends nothing reads
`running (no detail from <harness>)`.
The Chats list is where tasks are listed, and where every chat is: the explorer lists none
(#1673).
_Avoid_: job, todo (that is the workspace's), work item, task chat, sub-chat, persona chat (in a
shown sentence, where a task is meant)

**Ending a task** (by the person):
The two ways a person ends a task by hand, and no other: **Stop and get its report**, which
ends its turn and gives it one short turn to say what it did, and **Close now**, which ends its
program at once. Either takes a second step before anything ends. Closing a task's tab is
neither: the tab goes, and the task goes **back to the list** and keeps working. A chat's own
way to end a task it dispatched is a **cancel**.
_Avoid_: kill, terminate, close the task's tab (to mean ending it), stop chat (that is a
session's or a handoff's)

**Outcome** (of a task the asking chat is told of):
How a task ended, in purlis's own words to the chat that asked. Beside a report's own outcome
(done, blocked, failed, cancelled) there are four that are never a task's to say: **stopped by
the person** (with the task's short report, quoted as data), **closed by the person** (no
report), **ended without a report** (its program ended by itself), and **a report purlis made
from its last message** (its turns ended without a report after it was asked once: what it
last said, quoted as data; a report of its own later still reaches the chat that asked). The
first two carry one fixed sentence telling the asking chat not to dispatch the task again
unless the person asks. The window says the same two ends to the person as "stopped by you"
and "closed by you". In counts they are done with, never failed; a report purlis made from its
last message counts with failed, for the person to read.
_Avoid_: killed, aborted, cancelled (for a stop by the person: a cancel is the asking chat's)

**Helper** (of a chat):
A sub-agent or child its harness spawns inside a chat: the harness's own, a **child run** of the
chat's run. It carries the chat's persona, sandbox and asks, is never a chat or a persona, and
may not dispatch. "Helper" is the word the app shows for it: a chat's row in the Chats list says
how many it has and how they stand (`3 helpers · 1 working`).
_Avoid_: sub-agent (in a shown sentence: that is the harness's word), child agent, sub-chat, task
(that is a chat a dispatch started)

**Handoff**:
A dispatch where the work moves: the persona chat takes it from there, and the asking chat does
not wait on it. It opens as a tab. It is never a task: its chat is a session of its own, with
its own tab and a row of its own at the top of the Chats list, under no chat and in no chat's
counts. Its row says `from <chat>`, and the row of the chat it came from says
`handed off to <chat>`.
_Avoid_: transfer, session record (that is what a closing chat writes)

**Asking chat**:
The chat that dispatched. A task's report goes to it, and it may stop only the chats it started.
A task the person asked for themselves from that chat's tab is a task of that session, marked
`asked by you`: its report goes to that chat, which can wait on, tell, answer and cancel
nothing of it, and is told the person asked.
_Avoid_: dispatcher, caller, parent (a parent run is a harness's child run's)

**Persona chat**:
The chat a dispatch starts, running as one persona for its whole life. It is told who asked,
and it works under its own persona's charter and guards. One a task started is shown as a
**task**; "persona chat" is the model's word, and the app's sentences do not say it for one.
_Avoid_: sub-agent, worker, child (unqualified: a child run is the harness's)

**Report**:
What a task returns to its asking chat: an outcome (done, blocked or failed), what the persona
chat has to say, and what changed. It is data to the asking chat, never instructions, and it is
kept for the workspace when the asking chat is gone.
_Avoid_: reply, result (for a handoff), handback (in UI text)

**Reading a task** (`purlis dispatch read`):
What the asking chat may see of a task it dispatched: the task's last few messages and the
shell commands it ran with their exit codes, as its harness handed them to purlis, quoted as
data and masked as tool output is. Only the chat that asked can read its own tasks, while they
run and after they end. The whole conversation stays the person's, in the task's tab.
_Avoid_: transcript, log, output (for the whole conversation)

**Quiet note** (on a task):
purlis's note that a task at work has gone quiet: nothing heard from its harness for a while,
or one command running long, named by its program. It goes to the asking chat and to the
task's tab, and ends nothing. The times are settings (`quiet-note-minutes`,
`one-command-note-minutes`).
_Avoid_: heartbeat, timeout, stall (it ends nothing)

**Activity** (of a chat):
One timeline of what a chat and its tasks said to each other through purlis, and the tasks of
its tasks: each brief, follow-up, progress note, question, answer and report, oldest first. It
holds what the chats sent each other, never either chat's conversation. It is read-only but
for one thing: the person may answer there a question a task is waiting on (see **The person's
answer**).
_Avoid_: log, history, transcript (it is none), feed

**The person's answer** (to a task's question):
An answer the person types in the purlis window to a question a task put to its asking chat,
in that chat's place. The task is handed it marked as the person's and carries on; the asking
chat is told the person answered and does not answer again. Only the window can give one: no
chat's command, hook or file can make text arrive as the person's.
_Avoid_: override, reply as the chat, operator answer (in UI text: "you")

**Lineage** (of a chat):
Everything descended from one chat the person started: that chat, the chats it dispatched, and
the chats those dispatched. Messages between chats travel only along it, and every ask a chat
raises shows it.
_Avoid_: tree, family, parent (for an asking chat: a parent run is a harness's child run's)

**Dispatch grant**:
The person's rule that one persona may dispatch to another: for this chat, for me on this
machine, or for everyone in this project. Dispatching to the same persona needs none, and
nothing a chat sends can make one. It is one-way. **Any persona** is a grant with no named
target, made only in Settings; **never for this pair** is the person's refusal on their
machine, which no grant covers until they lift it. A grant holds **in any workspace** or **in
one workspace**: the workspace the dispatched task works in, never the one the asking chat is
in. A never is not limited. It counts only while both personas exist. Where a
persona was seen gone and another has its name, the grants for the name are **set aside**:
they allow nothing until the person gives them back or removes them. What a persona **wants**
(the `wants` line of its definition) is not a grant and makes none: it only puts unticked
boxes under the question, so the person can allow several pairs in one answer, each with the
workspace condition of that answer. A project grant a teammate committed **arrives**: it
allows nothing on a machine until the person there accepts it, and their yes holds only until
a commit takes the grant out.
_Avoid_: permission (that is the harness's), approval (that answers an ask)

**Refused while you were away**:
A dispatch a chat nobody was at was refused for lack of a standing grant, kept so the person
reads of it in the needs-you list afterwards and can allow the pair from then on, for work in
the workspace the refused task would have worked in. One item a pair and workspace, attached to
no chat; it is not a question and holds no dispatch. Dismissed, it stays put away while the
chat asks on.
_Avoid_: pending dispatch, queued dispatch (nothing is waiting to start)

**Headless chat**:
A chat with no tab yet. It is listed, never hidden: its asks reach needs you, Stop and the kill
switch reach it, like any chat's, and it gets a tab when the person opens it. Never a level-1
chat.
_Avoid_: background chat, unattended, hidden

**Peer message** (ADR 0090, proposed: not in the first version):
Words one chat sends another live chat, in its lineage or over a **message link**. It reaches the
receiver as quoted data at a turn boundary, and is never consent for anything.
_Avoid_: prompt (that is the operator's), message (unqualified), mail

**Mailbox** (of a chat; ADR 0090, proposed: not in the first version):
Whether a chat takes peer messages: deliver, hold or refuse, set by the operator. Held messages wait
on the chat's row.
_Avoid_: inbox (that is `purlis inbox`, a person's), queue

**Message link** (ADR 0090, proposed: not in the first version):
A person's leave for two chats, or the chats of two personas in a workspace, to send each other
peer messages outside their lineage, until it ends.
_Avoid_: link (unqualified: that is a runner's), channel, subscription

### The window

**Split window**:
An OS window a project tab was moved into, beside the main window. It holds its own projects,
and closing it moves them back to the main window with every chat still running.
_Avoid_: detached tab, pop-out, secondary window

**Welcome**:
What a window with no project open shows, always, whatever the machine remembers: three doors
(Create a project, Get a project from your forge, Open a folder on this machine…), a sign-in to
GitHub or GitLab, and the recent projects. A door opens its page in the welcome's place, and
Back returns to it (#1812).
_Avoid_: opener, first run, start page, splash

**Guided steps**:
The welcome's way to an open project, one step at a time with "Step N of M": welcome, sign in
(when taken), create, get or open a project, install git (when missing), pick your harness, open
the project. Where the person is, is kept per machine, so a quit resumes at the same step (#1817).
_Avoid_: wizard, onboarding (in the window's words), setup assistant

**Strip**:
One row of tabs: projects (in the title bar), a project's workspaces, or a workspace's chats.
A strip's order never changes on its own, and it never scrolls. The operator can drag a tab
along it; dropped among the pinned tabs it is pinned, and among the others it is unpinned.
_Avoid_: tab bar, scroller

**Side**:
One edge of a project's window: its activity bar, its sidebar beside the editor and its **Panel
half**, together. Left is navigation, right is what is for you (ADR 0038). A view moves only
within its own side.
_Avoid_: pane (a pane is a chat's terminal), column

**Panel**:
The full-width region under both sidebars and the editor, in a left and a right **Panel half**,
switched by the **Lower icons**. It holds reference lists, such as Repos, beside what the
sidebars hold. ⌘J hides and shows it; with both halves put away it is not drawn and the editor
has the height (ADR 0038, amended 2026-10-11).
_Avoid_: bottom panel, bottom bar, drawer

**Panel half**:
The left or the right part of the **Panel**, belonging to that **Side**. It shows one view at a
time, chosen by its side's **Lower icons**, and is put away on its own; the other half then takes
the whole width.
_Avoid_: bottom-left, panel section, tab group

**Lower icons**:
The group of an activity bar's icons below its divider, one per view of that side's **Panel
half**. A press opens the view in the half; a press of the open one puts the half away.
_Avoid_: bottom icons, panel tabs

**Pin**:
One operator's mark that a project, workspace or chat matters to them. It is kept on this
machine and never in the plane. A pinned item is drawn first, and the workspace strip draws
only pinned workspaces plus the one you are in.
_Avoid_: favourite, star, bookmark, pin (for a runner or a browser: that is **Pin** (of a
runner) or **Pin** (of a browser))

**Pin** (of a runner):
A human's act that gives an agent-started runner to its workspace: the runner is then owned by
the workspace instead of the chat, outlives the chat, and stops only by a human or the kill
switch. Kept in the runner's record on this machine. A part of **Runner**, so it belongs to
**Project** (ADR 0089). A browser is pinned to its workspace too, but a pinned browser ends
when the app quits (see **Pin** (of a browser)).
_Avoid_: keep, hold, pin (unqualified, which is the strip's mark)

**Show-more**:
The button at the end of a strip that lists what the strip is not drawing, sorted by activity,
with the needs-you count of everything it hides.
_Avoid_: overflow (in UI text), more tabs

**Needs you**:
A chat that is waiting on the operator: a view, computed from its current run (an ask, a turn
that ended, a budget or policy pause) and its own items (a report it wrote that has nowhere to
go, a refused commit, a secret waiting for approval), and never a state of its own. A report
that reaches the chat that asked is that chat's to read, and is not an item. An item shows on
its chat's row and on every row above it in the Chats section. Every project's are counted on the
title bar's ✋, which opens the **Inbox**, and each is counted in red on its tab and on any
show-more hiding it. **Ignore** clears a chat's items until the next one arrives (ADR 0076).
_Avoid_: notification, alert (an alert is a **Notice**), waiting (for the state)

**Component family**:
One kind of control the window draws one way everywhere, such as the button (with the icon button
and the toggle button), the field, the select or the dialog: its variants, its sizes and its
keyboard, built once in `app/src/ui/<family>` and used by every screen. A screen never builds a
control of a family by hand, and lint holds it (ADR 0104).
_Avoid_: component library, design system (that is the tokens, the families and their rules
together), widget, house component

**Window gutter**:
The gap between the window's regions, and around them inside its frame: a theme's
`window.gutter`, with `panel.corner-radius` for the panels' corners and `panel.border-width` for
the hairline between two flush regions. Purlis Classic's is 0, so its regions sit flush; Purlis
floats them (#2805).
_Avoid_: margin, padding, spacing (each a size inside one control)

**Status item**:
One item of the status bar, or a status pill at the title bar's right: an item purlis draws or
one an extension gives as data, on the bar's left or right end by its priority, which the person
may hide, and whose press runs a command (#2913).
_Avoid_: status cell, footer item, badge (a badge is an extension's fact on it)

**Primitive wrapper**:
The one file in a component family that wraps a React Aria primitive, the only place the
primitives library is imported. It is where the family's variants, sizes and parts are set, and
the seam a later change of library would cross (ADR 0104).
_Avoid_: adapter, shim, HOC

**Notice**:
A standing line in a project's window about something that is true now, such as a pin to a
workspace that is gone or a repo that could not be cloned. A Notice always offers a way out:
the fix itself when purlis can do it, or a link to the place where it is fixed. Dismiss
hides it until its cause changes. A reference to something gone is set aside, never removed,
and comes back when its target does. A project's Notices are listed in its **Inbox**, after the
asks; an alert purlis finds about a project or this machine is one of them (#1695). Not an
**Update**, which says what happened, and not **Needs you**, which is a chat waiting on the
operator.
_Avoid_: notification, banner, toast, message (for the thing itself)

**Away summary** ("While you were away"):
The one **Notice** each project of a window draws when the person comes back after five minutes
or more away from the window (not in use, hidden, or with no input), in place of reading what
each task did on its own: how many tasks finished done, how many failed, how many chats came to
need the person meanwhile (new to the needs-you list, or there already with something new), and
how many dispatches were refused while nobody was at their chat, each part a link to where it is
answered. It counts only what happened while the person was away. It hides nothing: every task
keeps its row and its **Activity** lines, and every question stays where it was asked. Going to
a failed task from it marks the failure looked at, as its needs-you item's Go does; dismissing
it dismisses nothing else. A setting turns it off. Away here is the person away from the window;
a chat nobody is at is another thing, whatever the window is doing.
_Avoid_: digest, recap, notification

**Ask**:
A chat's harness handing control to a human: a permission, a question, a request for values (an
ACP elicitation, which always elicits a secret), or a nudge. Every
harness's own form of it is read into one shape: what it would do, the options as the harness
offered them, who may answer, its deadline, its risk, a masked one-line summary, and whether it
elicits a secret. One answer per ask, and the first wins; every later one hears "answered
elsewhere". An agent never answers one (ADR 0080 §5). A part of **Chat**.

Everything that blocks until the person decides is an ask, whatever its source: a permission
prompt of a chat or a task, a dispatch grant, a host a chat's sandbox refused, a prompt waiting
in a harness's own terminal, a chat waiting on the person's reply, a chat's browser that needs
the person (**The browser needs you**). Each is derived from the
source that waits, keeps no record of its own, and is gone the moment its source stops waiting.
Each names its chain (the session first, the chat that asked last) and the path that answers it,
which is its source's own, and its answers in their one set of words (#1700). The Inbox lists
the asks oldest first, by when each began where its source knows. A chat's pane draws at most
two of its own asks as Notices, from the same list, so an answer in either place clears both
(#1695). The title bar's ✋ counts asks. Not an **Update**. A prompt in a
harness's terminal is answered from the window where the harness's hook can carry the answer (a
permission prompt of Claude Code, Codex or opencode); otherwise it is listed as waiting in its
terminal, naming the kind of prompt, with Go to chat.
_Avoid_: prompt (that is what the operator types), approval (that is one kind of answer)

**Inbox**:
The one place for everything that waits on the person: its **asks** first, then the project's
**Notices**, then its **updates** (spec #1688, #1695). The title bar's ✋ shows its count of asks,
and the status line's Notices item its count of Notices.
_Avoid_: queue, alerts, notifications

**Update**:
Something the person may want to know that waits on nothing: a task finished or failed, a doctor
finding, a chat that resumed, a sandbox change, a dispatch refused while nobody was there, a
Smart close that stopped, a report with nowhere to go, a commit refused (#1694), an Ask about a
chat's browser that ended with the browser (#2015). Listed in the
**Inbox** after the asks, newest first, and kept a day per
machine in purlis's data home, never in a project, so it survives a relaunch (#1693). Read and
dismissed one at a time or all at once (Mark all read, Dismiss all), and never counted on the ✋.
A task that failed is an update and not an **Ask**. Not an **Ask**.
_Avoid_: notification, news

**Kill switch**:
Stop all on the title bar, or `purlis stop --all`: every chat's and shell's program that purlis
started, in every project and window, is interrupted and ended, and no chat starts until the
operator **re-arms** it from the title bar. It is a stop, not a close: the tabs stay, each
reading as a chat whose program ended. A new shell still opens, so the operator can look around.
Nothing on the command line re-arms (ADR 0071).
_Avoid_: panic button, pause (nothing is resumed on re-arm)

**Shell tab**:
A tab running the operator's own shell, with no harness and no profile, opened by `New shell`.
A harness typed into one runs outside purlis's session tracking, so purlis's **shell-tab
shims** stand first on its `PATH`: the harness still starts, after one line saying so, and the
tab shows a banner offering to open it as a chat instead (ADR 0062).
_Avoid_: terminal (for the tab), console, plain chat

**Light editor**:
purlis's one editor, for reading a file of a chat's branch, making a small edit in it and
reviewing a diff. It has no language server, debugger, repo-wide refactor or extension code of
anyone else's; deep work opens in **your editor** at the same file and line. It is a view of a
**Workspace**'s repos (ADR 0072 §2, ADR 0081).
_Avoid_: editor (unqualified), code editor, IDE (except in the category phrase "the agent IDE",
which is purlis as a whole)

**Your editor**:
The editor the operator already uses (VS Code, Zed, a JetBrains IDE, or `$EDITOR`), where writing
code by hand happens. purlis opens a file there at a line, and never replaces it. Which editor
is a setting of this machine, so it belongs to **Project** (ADR 0072 §2, ADR 0081).
_Avoid_: external editor; IDE (except in the category phrase "the agent IDE")

**Comparison**:
A base and a head in one of a workspace's repos, which git turns into the diff a Review tab shows: a
chat's branch against its base, two refs, uncommitted work, one agent turn, a request. A
cross-repo change is one comparison per member. It is a part of a **Workspace** (ADR 0084).
_Avoid_: diff (for the pair; the diff is what git computes from it), changeset

**Review tab**:
A view tab showing one comparison (or one per member of a cross-repo change), where the operator
reads the diff, ticks files as viewed, comments on lines, and ends by sending the comments to the
chat, publishing them to the request, or approving. There is one per branch, and it remembers
where the operator was. The forge's own review of a request stays the review of record. "Review"
on its own is the plain verb, as on the chat tab's button, and is not a purlis noun. It is a view
of a **Workspace**'s repos (ADR 0084).
_Avoid_: Review (as a noun for the tab), code review (for purlis's), PR review (that is the
forge's), diff view

**Review draft**:
A Review tab's unsent comments: each one's line and the operator's words, never code. Only the
window writes it, it is kept on this machine and travels with the operator's other machines,
and it leaves only by **Send to agent**, **Publish** or being kept. It belongs to **Workspace**
(ADR 0084).
_Avoid_: pending review (that is GitHub's), draft (unqualified)

**Human edit**:
An edit the operator saved to a file in a chat's branch from the light editor. purlis records
which file and lines, and announces it to the chat's harness at its next turn: one line of
context naming the files edited since its last turn, paths only, never contents. Edits from
several sittings add up until that turn, and the announcement is never a prompt sent for the
operator. It is a part of a **Chat**'s history (ADR 0084).
_Avoid_: manual edit, override

**Session record**:
A summary a chat writes of its own session when it closes through **Smart close** — its goal,
what it did, what it decided, what is still open and how to pick it up — filed as one file in
its workspace's `sessions/` (the plane's own, at the plane root). The chat gives the title and
the five sections; purlis gives everything else (which chat, persona, harness, profile,
conversation, workspace, directory and pieces), keeps the index and `workspace.md`'s one `## Sessions` line, and names
the newest in the next chat's briefing (ADR 0064).
_Avoid_: handoff, log, transcript

**Smart close**:
Closing a chat after it has written its session record: the app sends it one line naming
purlis's `smart-close` skill (the operator's click is the consent, the exception to a curation
action's never-sent prompt), and the tab closes when `purlis session record` tells the app the
record is saved — never on anything the chat printed. Until then the chat is **wrapping up**:
its tab says so, and the operator's Cancel smart close or their own typing stops it.
_Avoid_: save and close, archive

**Smart-close pass**:
What a person's own start of a smart close gives that chat: the tab's **Smart close**, or the
person typing `/smart-close` into a waiting chat. It is bound to that chat and that close, ends
with it, and is the only thing that lets a record close the tab. A chat can't give itself one: a
typed `/smart-close` counts only beside the person's own Enter in the chat's pane (#1332).
_Avoid_: token, ticket (that is a handoff's), approval

**Resume** (of a session record):
Starting a NEW chat from a session record, in the record's place — the directory it ran in,
where that is still in the place — on its harness and the profile it ran on where this machine
still has it, given its conversation where the harness can still find it, and with the record
quoted in its briefing. What it had to guess instead, it says.
Where the conversation cannot be given it is a fresh chat with the record, and it says why. It
never reopens the chat that wrote the record.
_Avoid_: reopen, restore (a relaunch reopens the chats that were open)

**Plane updated** (of a chat):
A chat started before the plane's start-time instructions (`CLAUDE.md`, the harness settings
and sub-agents, a persona's charter) changed on disk. It runs on what it read until it is
started fresh, and its tab carries a quiet mark saying so. It is not a needs-you item.
_Avoid_: stale, behind, outdated, Incoming (that is the remote's commits)

### Themes

**Theme**:
What the window is drawn in: every token's value, from colour and motion to geometry, fonts, the
icon set and how a workspace's tint lands. A theme is bounded data, never code, and may set every
token; what it leaves out comes from its parent or from core (ADR 0097). A theme is built in, the
person's own, a project's, a workspace's or an extension's.
_Avoid_: skin, look (as the name), colour scheme (that is only its colours), style

**Theme folder**:
A theme as it is kept: a folder holding `theme.json`, and optionally its fonts, icons, a preview
and a licence. It is data, not a capability (ADR 0098). A project keeps its theme folders in
`themes/` at the plane root, and a workspace in its own `themes/`.
_Avoid_: theme file (one file is not a folder), theme package, theme pack (that is an extension)

**Appearance**:
One way a theme is drawn, tagged dark or light and optionally high contrast. A theme has one or
more, and the person picks one or lets the system choose. A theme with a dark and a light
appearance can follow the system.
_Avoid_: mode, variant, flavour, dark mode (as the name of the setting)

**Theme option**:
A toggle or a choice a theme offers, such as an accent or borders on or off, that sets tokens
when picked. Its values are the person's choice. An option never sets a free number.
_Avoid_: setting (that is the person's, for every theme), variant, preference

**Component part**:
A named, stable element of a component family, such as the button's root, label or icon, that a
theme may set on its own: its fill, border, corners or size, within bounds, keyed
`<family>.<part>[.<variant>][.<state>].<property>`. Each family declares its parts once, in the
window's wrapper and in the theme engine's closed, versioned manifest (ADR 0103). The button's
`grants` variant is the look of an answer that grants (an Allow, an Accept), not a grant itself.
_Avoid_: part (bare, which is plain composition here), slot, sub-component, element

**Design-tokens file**:
A theme folder written in the W3C Design Tokens Community Group's format, version 2025.10
(`.tokens.json`), for exchanging a theme with a design tool: `purlis theme export --kind
design-tokens` writes one and `purlis theme import` reads one back to the same folder. It is a way
in and out, never a theme format purlis draws from; `theme.json` is that.
_Avoid_: token file (a theme's tokens live in `theme.json`), tokens export

**Palette** (of a theme):
A theme's named colours, which its tokens refer to by name, so one colour is changed once. The
names belong to that theme alone.
_Avoid_: palette (bare, which is the command palette), swatches, colour list, primitives

**Density**:
The person's setting of how much room the window gives its rows and controls: comfortable or
compact. Every theme honours it, and a theme may tune what compact means for it.
_Avoid_: compact mode, zoom, scale, text size (that is a setting of its own)

**Theme specimen**:
A fixed page drawn off-screen with sample content, in a given theme, appearance and options. It
is how a chat sees a theme it makes and what the gallery's cards show. It never shows the person's
real window or data.
_Avoid_: preview (that is drawing a theme in the real window), screenshot, sample window

**Theme check**:
What purlis says about a theme: format and bound errors, contrast findings by pair with the
ratio, and hard-floor findings, for every appearance and option combination. A format error
refuses the theme; a contrast finding is a warning.
_Avoid_: theme lint, validation, audit

**Grey tier**:
One of the steps a theme draws its greys in: four for words (primary, secondary, muted and quiet,
each held to 4.5:1) and faint for what is not words and for disabled words (held to 3:1), each
measured on every ground a part draws it on. Quiet is informational text; it is never faint.
_Avoid_: text colour (a tier is a rank, not a colour), dim, disabled colour

**Theme draft**:
A chat's unsaved changes to a theme, drawn only in that chat's workspace (the plane root for a
root chat) until the person keeps or reverts them. A strip on the workspace says it is live and by
which chat.
_Avoid_: preview, unsaved theme, working copy

**Hard floor**:
A rule about readability no theme draft can break, such as primary text at 3:1 or a visible focus
ring. A draft that breaks one is not drawn, and the last good version stays.
_Avoid_: contrast check (that is the theme check's), minimum, guard

**Workspace theme**:
The theme a workspace is drawn in, which the window repaints to when the workspace comes to the
front. It is the person's choice for that workspace, or a LIVE workspace's committed default.
_Avoid_: workspace colour (that is its tint), workspace skin

**Foundation**:
The designer's drawn look that the theme Purlis is built from: its type, colours, icons and
sizes, kept as a record in the project.
_Avoid_: design system (that is how the window is built), brand, mockup

**Default theme**:
The theme a level offers when the person has chosen none: the project's or a LIVE workspace's
committed one, or Purlis when no level says. The person's own choice always beats it.
_Avoid_: fallback, standard theme, base theme (that is what a theme extends)

**Preview theme**:
A shipped theme that is not the default yet and says so in its name, as "Purlis (preview)" did
until Purlis became the default (#3034); none ships as one now. It is chosen like any theme and
moves nobody to it.
_Avoid_: beta theme, experimental theme, preview (that is drawing a theme in the window)

**Purlis Classic**:
The look purlis had before Purlis became the default (#3034), kept unchanged as the alternative
built-in theme, in a dark and a light appearance, picked by its id, `purlis-classic`. The ids
`purlis-dark` and `purlis-light` name Purlis's dark and light.
_Avoid_: purlis-dark, purlis-light (those are old ids), legacy theme, old theme

### Saving

**Save**:
Taking what changed in the plane or a repo as far as its mode allows: commit, push, PR, merge.
_Avoid_: sync (that word is `purlis sync`'s), commit (a save may be more than one), publish

**Sync**:
Fetching every clone in a workspace and fast-forwarding the ones that hold no work:
`purlis sync`, and *Sync repos* in the window. Nothing else is called Sync: not a save, not
moving a project to its pinned version, not writing persona sub-agents, and not any state kept
between devices (ADR 0072).
_Avoid_: pull, refresh, update (for this)

**Mode**:
How far a save goes: `off`, `commit`, `push`, `pr` or `pr-merge`. Each value includes the
steps of the one before it. The plane has one mode, and each repo has its own.
_Avoid_: policy, posture, share

**Auto-save**:
A save purlis starts by itself: after a quiet period, when a session ends, or when the app
quits.
_Avoid_: sync, background push

**Target branch**:
The branch a save is meant to end up on.
_Avoid_: base branch, main (it need not be)

**Save branch**:
The one branch per clone of the plane (named for the machine and the clone) that the plane's
request modes push to, carrying one open request into the target branch.
_Avoid_: PR branch, `charter/<sha>` branch

**Request mode**:
A save mode that keeps one request open into the target branch instead of pushing to it: `pr`,
and `pr-merge`, in which purlis also merges the request itself once every check on it has passed.
A red request stays open and is a **Notice** of the kind `save-request-red`, in the Saving view
and the **Inbox**, with two answers: **Fix in a chat**, which starts a chat at the **plane root**
(or in the repo's workspace) on what failed, and **Merge as it stands**, the person's own merge
past the checks, which asks first.
Prose says "request mode" on every forge; `pr` and `pr-merge` stay the words `charter.toml` takes
(V81).
_Avoid_: PR mode, MR mode

**Stage**:
Where unsaved work sits. It is *changed* (not committed), *committed* (not pushed), *pushed*
(PR open), or *saved* (on the target branch).
_Avoid_: status, sync state

**Blocked**:
A save that can't go further without a person: a conflict or a merge or rebase git stopped
part-way, a refused push, a secret the scan caught, a mode the remote can't take. Auto-save pauses until it's cleared.
_Avoid_: failed, error, stuck

**Incoming**:
Commits on the remote that this machine doesn't have yet.
_Avoid_: behind (in UI text)

**Commit scan**:
The check a chat's own commit passes before git makes it: the lines it adds, scanned for keys
and personal data, and refused with each finding masked. `purlis scan` runs it on what is
staged. Part of Workspace, as a check on a repo's save (ADR 0074).
_Avoid_: secret scan (that is the plane save's), leak check

**Allowlist** (of the commit scan):
A repo's `.charter-scan-allow.toml`: the findings the commit scan lets through, each entry with
its reason, committed by the operator and read as it is at `HEAD`. A setting of a repo.
_Avoid_: ignore list, exceptions, whitelist

**Shared / Local** (settings):
Where a setting's value comes from: `charter.toml` (committed, the team's) or
`charter.local.toml` (this machine's). A Local value overrides the Shared one key by key.
In Settings a value says which it comes from, and "Shared / Only on this machine" moves it
between the two.
_Avoid_: global/user, project/personal

**Settings**:
The one tab where every setting of purlis is read and changed, at one level at a time. Each
group of settings has its own place in it, and anything that tells you to change a setting can
open it at that place. Opening it from a project or a workspace opens it at that level.
_Avoid_: Preferences, options, config page, project settings page (as a separate thing)

**Level** (of settings):
Whose setting it is: **You** (this machine, for every project), **Project**, **Workspace** or
**Persona**. A level's value overrides the one beneath it, and each value says which level and
which file it came from.
_Avoid_: scope (that word is kept off settings), layer, tier (tiers are where stores live)

### The sandbox

**Sandbox level**:
How much a chat's sandbox holds, shown on its tab: **full sandbox** (a project's `mode = "on"`:
the hosts it lists, the folders a chat writes, the asks), **floor** (the default, #2071), or
**no sandbox** (a person's choice for a chat, or where the floor cannot be applied).
_Avoid_: mode (that is the file's key), strict, relaxed

**Floor**:
The sandbox a chat runs in where its project has not turned the full sandbox on: every host
reached through the chat's own proxy and none asked about, every folder written but what it
holds. It holds the denial classes (no vault or purlis key read, nothing a later program runs
written, at the person's own level too), the folders on the person's `PATH` and the programs
it starts, which a block's Allow can lift, and no program started outside it, no local socket
and no port of this machine but its own proxy's.
_Avoid_: light sandbox, partial sandbox, sandbox off

**Brokered write**:
A write to the project's own files that a sandboxed chat asks purlis to make for it: a session
record, memory, todos, `workspace.md`, persona files, a workspace or handoff made, and the
clone, checkout or worktree a sandbox would refuse. purlis makes it as the terminal would, and
the chat's sandbox does not widen. A chat writing one of those files itself is refused and told
which tool to use. A change to `workspace.md` or a persona charter shows a **Notice** with Review
and Revert (ADR 0067 §2).
_Avoid_: escalation, unsandboxed write, proxy write

**Internet access**:
Which hosts a sandboxed chat can reach: readable presets ("AI providers", "Code hosting",
"Package registries"), each listing its hosts, plus the project's own hosts and your personal
ones. The project's choice is committed and every teammate follows it; a host nothing allows is
blocked and shown (ADR 0067 §1, §3).
_Avoid_: hosts it may reach, egress (in UI text), allowlist (that is the commit scan's), network
policy

**Open hosts**:
The hosts every sandboxed chat in a project reaches without asking: each **Internet access**
preset's hosts and the project's own hosts. Settings' Network page spells them out host by host
(spec #1661).
_Avoid_: allowlist, egress (in UI text), default hosts

**Persona hosts**:
The hosts only one persona's chats reach, once the person has allowed that persona's list on
this machine. The project commits the list with the persona; each machine allows it on its own.
_Avoid_: persona grants (in UI text), persona allowlist

**Allowed host**:
A person's yes to one host and port at one scope: *this chat*, *this project on this machine*,
or *everyone in the project*, which is committed so teammates follow it. It is shown in full,
never as a wildcard, and every one can be removed from Settings' Network page.
_Avoid_: grant (in UI text; that is a runner's **Grant**), exception, allowlist entry

**A service on this Mac**:
Whatever listens on one port of this machine's loopback interface: a local Kubernetes cluster, a
database in a container, a dev server. A sandboxed chat reaches none of them until a person
allows that one port, for *this chat* or for *this project on this machine*; never a range,
never every port, never one of purlis's own. Allowed from the Notice its refusal raises, or
added by its port in Settings' Network page, which refuses exactly what the Notice refuses. Kept
on this machine only, listed in Settings' Network page with Remove. A Claude Code chat reaches it through its proxy by the name
`this-mac.internal:<port>`, and its kubectl through a copy of one context purlis makes for it.
_Avoid_: localhost (in UI text), local port grant, loopback

**A proposed change**:
An edit a chat makes to purlis's own configuration (the project's manifest, a persona's
`mcp.json` or `persona.md`, a skill's files), which only the person applies, in the window. The
chat's edit is held, the change shown whole against the file as it is, and **Apply** writes it as
the person; **Dismiss** writes nothing. Distinct from an edit ask, where the person allows and the
harness writes.
_Avoid_: patch, suggestion, edit ask (that is the harness's own)

**Block**:
One refused reach: a sandboxed chat was refused a host and port, a local socket, a service on
this Mac, a lookup or a file. purlis records the chat, what was refused, the time and why in this machine's network
record, kept 30 days and never committed, shows a **Notice** on the chat's tab, and lists it
under Blocked lately.
_Avoid_: denial, violation (that is the sandbox's own report), egress refusal

**Policy**:
An admin's locks on settings, kept on this machine or for the organisation. A locked value
cannot be changed at any **level**, the strictest value wins, and Settings shows it as "locked
by policy". It never loosens what a project or a person sets. One that forbids the opt-out
requires the sandbox in every project on that machine, which Settings shows as "On, required by
policy" (ADR 0067 §1, §4 as amended).
_Avoid_: supervisor, managed tier (in UI text; that is a harness vendor's), org settings

### Core and extensions

**Core**:
What purlis does itself, on every platform, with no extension on. A plane's instructions and
the session-start briefing may depend only on the core.
_Avoid_: built-ins (for core features), platform

**Extension**:
A directory the operator installs and approves on this machine, whose manifest declares what it
contributes and which capabilities it asks for. Its program runs as the operator, one question
at a time, except an extension that declares a **studio capability**, which runs confined in a
chat's sandbox profile: it reads and writes only its own folder and what purlis hands it, and
reaches only the hosts it declares, never a model provider's (ADR 0101).
_Avoid_: plugin, add-on, module

**Built-in extension**:
An extension that ships inside the app and is trusted through the app's signature rather than
an approval prompt. A copy of one anywhere else is an ordinary extension.
_Avoid_: bundled plugin, first-party plugin, core extension

**Capability**:
One thing purlis does for an extension that asked for it in its manifest and was approved,
such as showing a badge or adding a CLI command. It describes purlis's conduct. It limits what
the extension itself can do only for a **studio capability**, whose extension is confined
(ADR 0101).
_Avoid_: permission, grant (that is a runner's **Grant**), power

**Studio capability**:
One of the six capabilities the Skill studio is built from: SkillSource, Grader, SkillLint,
SkillTemplate, StudioPanel and SkillRouter. Each has a built-in first user. An extension that
declares one is confined, and they are experimental until the studio's finish line (ADR 0101).
_Avoid_: studio extension point, skill capability

**SkillSource**:
The studio capability for a source of skills that is not git, such as a registry or an artifact
store. It hands purlis a skill's folder at an immutable revision; the review, copy and pin are
purlis's (ADR 0100). The built-in git source is its first user.

**Grader**:
The studio capability that is the extension form of a **grading framework**. It gets a skill and
its fixtures and returns scores, and every chat it needs, a judge's included, is one purlis runs
for it. The built-in runner is its first user.
_Avoid_: eval engine, judge (a judge is a role a chat plays in a grade)

**SkillLint**:
The studio capability that adds findings to a skill's live lint and install review. It never
edits a skill. The built-in lints are its first user.

**SkillTemplate**:
The studio capability that offers starting points for a new skill, which lands as a draft skill.
The blank and starter templates are its first user.

**StudioPanel**:
The studio capability that adds a panel to a skill's tab, drawn with panel blocks. The Runs and
Grades panels are its first user.

**SkillRouter**:
The studio capability that is the extension form of a **skill router** (ADR 0102). The built-in
router is its first user.

**Facts file**:
A file an extension keeps in its own state directory, holding the values purlis shows for it
(badges, repo cells) without starting its program.
_Avoid_: cache, status file

**Event**:
One question purlis asks an extension after a core action it hears about has finished, such as
a workspace being created or the plane being saved. What it answers never changes the action.
_Avoid_: hook (for this), notification, subscription

**Briefing section**:
Text an extension adds to a chat's session-start briefing, quoted as data under the
extension's name. It is never an instruction, and never a permission, a hook or a setting.
_Avoid_: prompt, context injection

**Action** (of an extension):
A verb an extension declares and offers on the rows of its views: pressing one asks its program
to *run action `<id>` on `<subject>`*. Never one of purlis's own verbs. purlis asks first when
the manifest says so, and always before one that deletes.
_Avoid_: command (for this), verb (unqualified), button

**Curation action**:
A chat purlis opens on a workspace, a persona or the plane with a prompt already typed into it
and never sent: the operator reads it and presses Enter. purlis ships four of its own
(`charter/safe-remove`, `charter/compact`, `charter/add-curation-action`,
`charter/create-theme`), and a persona
declares more as `personas/<name>/curation/<id>.md`, which that persona runs. Unlike an
extension's **Action**, nothing runs a program: the chat is the whole of it (ADR 0061).
_Avoid_: action (unqualified), quick action, macro

**Palette command** (of an extension):
A row an extension adds to the palette, named with the extension's name, that opens one of its
views or runs one of its actions.
_Avoid_: shortcut, menu item

**Extension command**:
A command an extension adds to the `purlis` command line, run as `purlis <extension id>
<command> …`. It says whether it writes, and what its program prints and its exit status reach
the caller unchanged. An extension's id is never one of purlis's own command words.
_Avoid_: subcommand (unqualified), plugin command, palette command (for this)

**Core-owned alias**:
A core command whose words forward to an extension command and give its output, so a plane's
instructions keep working when a feature moves into an extension (`purlis ws todo` once todos
does).
_Avoid_: shim, redirect

**Write paths**:
The plane-relative paths an extension declares it writes. purlis hands them resolved with
each request and reports a change outside them; it does not stop one.
_Avoid_: sandbox, allowed paths, scope (as if enforced)

**Harness adapter**:
purlis code that arms one harness through its own mechanism for one chat, with nothing written
into the harness's config: what level 2 and a harness's own protocol need. It never stands in for
the harness's program (ADRs 0050, 0073).
_Avoid_: wrapper, driver, plugin (that is the harness's)

**ACP adapter program**:
A program the user installs that speaks ACP for a harness that does not, such as
`claude-agent-acp` or `codex-acp`. purlis spawns it as a level-3 chat's program, found by name
on `PATH`, and never ships, downloads or updates one. It is not a harness adapter, which is
purlis's code (ADRs 0073, 0080).
_Avoid_: ACP adapter (on its own), harness adapter (for this), bridge

**Wrap**:
To run a chat's unmodified harness inside a sandbox profile or backend purlis generates (ADR
0067). Never to **stand in** for the harness: putting purlis's own program where the harness's
is expected and changing what it or its model sees, which purlis never does. X34's *"wraps a
harness binary"* means standing in (ADR 0073).
_Avoid_: wrap (for a stand-in, a shim or an adapter)

**Harness plugin**:
A Claude Code, Codex or opencode plugin, chosen per project. "Plugin" on its own always means
this, never a purlis extension. purlis's own is one too: the Claude Code plugin the app
bundles, named `purlis` (`purlis@inline`, skills `purlis:<skill>`, MCP tools
`mcp__purlis__<tool>`), always on in the chats the app starts. Its ids from before the rename
(`charter@inline`, `charter-app@inline`, `charter@charter-app`) are always off there, and so
is the Python charter's `charter@charter`. `purlis plugin install` puts a copy of it,
`purlis@purlis-app`, in front of the chats the operator starts outside the app (ADR 0057). For opencode, purlis's own is the **opencode
shim**, a script the app loads into each opencode chat it starts, and whose guard-only variant
`purlis plugin install` writes into opencode's plugin directory (ADR 0058).
_Avoid_: extension (for this); "purlis plugin" for anything but purlis's own

**Skill**:
A folder in the open SKILL.md format: instructions with a name and a description, and optionally
scripts, references and assets. It works unchanged in any harness that reads the format.
_Avoid_: Claude Code skill, command, prompt

**Built-in skill**:
A skill that ships inside purlis itself, in its plugin (`skills/` in the bundle), one source for
every harness. It cannot be turned off or edited. Each harness is handed the built-in skills by
its own route, for the chat alone: Claude Code loads the plugin, opencode is told the directory
through the shim, and a Codex chat is **briefed** on them, a list of names, descriptions and
`SKILL.md` paths at `SessionStart` (ADR 0063).
_Avoid_: purlis's skills (every skill a person makes is a skill in purlis too), system skill,
"Claude Code skills" for these; a copy of them anywhere

**Skill set**:
The skills one chat gets, each with the route its harness is handed it by and where it comes
from, and every skill left out of it with a one-line reason. `purlis-skills` alone works it out
(#2762); a harness adapter only turns it into argv, env and config.
_Avoid_: loaded skills, available skills

**Skill studio**:
The one place where every skill a chat can get is listed, made, edited, installed, removed,
observed and graded: a view tab, with each skill opening as its own. It is built behind the
`skill-studio` flag until its **finish line** (spec #2762).
_Avoid_: skill manager, skills page

**Finish line** (of the Skill studio):
The set of things that must all work before the Skill studio is shown to anyone. Until then it
stays behind its flag.

**Harness skill**:
A skill a harness brings on its own, from the person's harness folders or from a harness plugin.
purlis lists it and can turn it on or off for a project, but never edits it.
_Avoid_: external skill, plugin skill

**Repo skill**:
A skill committed inside a repo a workspace clones, which the harness reads natively. purlis
lists it and may turn it off, but never edits it.

**Project skill**:
A skill owned by the project, a folder in the plane's `skills/`. It is usable in chats at the
**plane root**, and in a workspace's chats only where the project grants it (ADR 0099).

**Workspace skill**:
A skill owned by one workspace, in its `skills/`, usable only in that workspace's chats.

**Persona skill**:
A skill a persona loads wherever it runs, whether the persona owns it or names one owned
elsewhere. It is not limited by the grants of the workspace it runs in.

**Local skill**:
A skill kept on the person's machine and not committed, so their team never gets it. Sharing it
makes it an ordinary skill at the same level.
_Avoid_: private skill, draft

**Grant** (of a project skill):
A project skill's statement of where it may be used: the plane root only (`root`, the
default), every workspace (`all`), or a named list. A workspace can turn a granted skill off,
never on one it was not granted (ADR 0099).
_Avoid_: permission, share, scope

**Manual-only skill**:
A skill a harness never picks by itself. It runs only when it is asked for by name, and its
description stays out of the chat's context.
_Avoid_: hidden skill, disabled skill

**Draft skill**:
A new skill, or a change to one, waiting in the Skill studio for the person to accept. It is
usually written by a chat.
_Avoid_: pending skill, proposal

**Try it** (of a draft skill):
A scratch chat that has the built-in skills and the draft alone, started in a **throwaway
piece** on the project's default harness profile or one the person picks. Closing it leaves
nothing but the draft's skill run (#2772).
_Avoid_: test run, preview chat

**Throwaway piece**:
A folder made for one chat, ignored by git and never saved, and removed when that chat closes. Try it
starts its scratch chat in one, and grading's chats will too. On screen it is a **throwaway
folder**.
_Avoid_: scratch workspace, sandbox (for this), worktree (it is no branch)

**Deprecated skill**:
A skill that still works but points to a replacement and is shown less.

**Archived skill**:
A skill kept in history that never loads until it is restored.

**Pinned skill**:
A skill a persona or workspace keeps always loaded, so no router hides it.

**Skill owner**:
A person named on a skill whose review a teammate's change needs on an org plane.

**Skill source**:
A git repository of skills, or a source an extension provides (SkillSource), that skills can be
installed from. An installed skill is a pinned copy, updated only through a reviewed change
(ADR 0100). It is not a **marketplace**.
_Avoid_: marketplace, registry, feed

**Marketplace**:
The listing service of packs, extensions and personas (M47). A skill source is not a
marketplace.

**Skill run**:
One use of a skill in a chat, from the moment it is invoked until the turn ends. What happens
inside it counts toward the skill.
_Avoid_: invocation, call, execution

**Skill router**:
What decides which skills a chat sees or can load for its task. One ships with purlis; others
arrive as extensions.
_Avoid_: skill selector, dispatcher (dispatch is persona work)

**Skill budget**:
How many skills, and how many characters of their descriptions, one chat's listing may hold on
its harness. purlis sets it below each harness's own limit and shows it on the chat tab; past
it, the skill router hides skills behind the index skill (ADR 0102).
_Avoid_: context budget (for this), skill limit

**Use skill…**:
Picking one of a chat's skills, from the palette, the chat's tab menu or the Skills chip on its
pane, and having it typed into the chat in its harness's syntax, with the arguments the skill
declares, for the person to send. It lists exactly that chat's skill set, manual-only skills
included, and its run starts when that prompt is sent (#2775). A skill the skill router hid
behind the index skill is typed as a request to load it with `skill_load`, and its run starts
with that load (#2778).
_Avoid_: skill picker (for the act), run skill, invoke

**Index skill**:
An always-loaded skill a skill router shows whenever it hides any skill, listing every hidden
skill by name and one line and saying that `skill_load` loads it (ADR 0102).
_Avoid_: catalogue skill, skill menu

**Grading framework**:
A pluggable way of grading a skill. purlis ships one; others arrive as extensions.
_Avoid_: eval engine, judge

**Trigger test**:
A grade of whether a harness picks the skill when it should and leaves it alone when it should
not.

**Outcome eval**:
A grade of how well a skill does its job on fixture tasks.

The skills' relationships: a **project skill** reaches a workspace only through a **grant**; a
**persona skill** adds to what the chat has where it runs; when two levels hold a skill of the
same name the narrower wins (persona, then workspace, then project, and local over shared), and
a built-in skill's name is never taken; a **grading framework** produces **trigger tests** and
**outcome evals**, and **skill runs** supply the live signals.

**Vault**:
A named set of secrets purlis keeps in the system keyring and hands to a command, never to
the model and never to an extension. Vaults are core.
_Avoid_: secret store, keychain (as the name of the concept)

**Forge extension**:
An extension about a code host's pull requests, merge requests or issues, which reaches the
forge through `gh` or `glab`'s own login and never through a secret purlis hands it. Once
PE-29 opens the forge seam to extensions, it asks purlis to make the call instead (ADR 0070).
_Avoid_: forge plugin, GitHub integration

**Editor integration**:
purlis's own extension for VS Code or Zed, or its own plugin for JetBrains IDEs: installed in
**your editor**, it opens a file's chat in purlis, sends a selection to a chat's prompt for the
operator to send, and shows a chat's changes. It speaks the **editor protocol**, and is neither a
purlis **extension** nor a **harness plugin**. It is a view of a **Workspace** and its chats,
drawn in your editor (ADR 0072 §2, ADR 0081).
_Avoid_: extension or plugin (on their own), IDE plugin

### How purlis is built

**Dev channel build**:
The build a green `main` publishes to the dev channel: compiled with the lighter `dev-release`
profile, and with no `.dmg`. It is signed, checked and isolated exactly as a stable build is,
and it is for trying a change, never for measuring one (ADR 0092).
_Avoid_: nightly, debug build, fast build

**Stable build**:
The build a `v*` tag publishes to the stable channel: compiled with Cargo's `release` profile,
with every bundle. Every performance budget is measured on this profile (ADR 0042, ADR 0092).
_Avoid_: release build (that is the profile, and both channels' builds are releases of a kind),
production build

**Profile guard**:
The nightly build of `main` with the stable build's profile and bundles. It holds no signing key
and publishes nowhere, and a failure opens one issue or rewrites it. It is what shows the stable
build still works now that the dev channel no longer uses its profile (ADR 0092).
_Avoid_: nightly release, canary, stable dry run
