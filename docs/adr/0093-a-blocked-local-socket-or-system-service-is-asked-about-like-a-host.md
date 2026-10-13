# A blocked local socket or system service is asked about like a host

**Accepted 2026-10-12.** Amends ADR 0067 §2 ("each compiled profile allows the chat to connect to
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
  judges it again as a grant before anything is kept.
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
  service is chosen only from the ones purlis can allow, today the certificate check: a
  service has no path to check, so none is typed.
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
