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
  a second hard link: a chat could otherwise put any socket there under a harmless name. Each
  is judged by its resolved path when offered, when allowed, and again at every start; a kept
  path that is now a folder, a file, a linked socket or somewhere a chat writes is dropped. A
  container engine makes its socket anew each time it starts, so a grant is of the path, not
  the inode.
- **Where a grant is kept** (D-sock-2). For one chat, in memory, as a host's is. For this
  project on this machine, in the person's own config home, `sandbox-grants/<clone key>.json`,
  which no sandboxed chat writes and no repository brings. **Never in the project**: a
  repository could grant itself a socket that way. So no "everyone in the project" level is
  offered. And **a state file the repository brought grants nothing** (D-sock-9): an
  `app/sandbox.json` git tracks, or one the checked-out commit holds, is read as empty, so its
  folders, hosts and every other grant in it are asked about again.
- **Allow once** (D-sock-3). A socket or a service reaches a chat only through the sandbox it
  starts with, so a grant takes effect when the chat restarts on its conversation. "Once" is
  that run alone: it goes when the session it was allowed in ends, which the restart that takes
  it outlives, so no later start of the chat has it. It is offered for a socket or a service
  only: a host is asked about live, and a folder is written for longer than one run.
- **A declaration** (D-sock-4). `[sandbox].expects` in the committed file lists the sockets and
  services the project expects. It changes the words of the Notice ("This project expects it.")
  and marks the grant in Settings' Allowed list. It grants nothing; a test plants one and checks
  that nothing is compiled from it.
- **Detection** (D-sock-5). Claude Code's `<sandbox_violations>` lines, purlis's own Seatbelt
  log, Go's `dial unix <path>: connect: operation not permitted`, and Docker's own sentence on
  macOS each name the socket; a `mach-lookup` violation names the service; Go's `x509: OSStatus
  -26276` names the certificate check. A failed command's output names its socket too, since
  that is the only place a client says it; the Notice shows the path whole and judges it again
  as a grant (a socket on this machine now) before anything is kept.
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

## The committed `certificate-checks` (a behaviour change)

**`[sandbox].certificate-checks` now declares the certificate check and grants nothing**
(decided under the operator's rulings 2 and 3: committed files declare, never grant; the
certificate check is ask once, then always). A project that sets it gets the Notice's words
"This project expects certificate checks", and each person allows it once on their machine the
first time a Go tool checks a certificate, Always allow among the choices. Projects that set
it now get one Allow per machine the first time a Go tool checks a certificate.

## What is not done

- **Brokered Docker**, purlis running `docker` for the chat after checking the command, is a
  safer route than granting Docker's socket, and a later, optional follow-up (#1807).

## Consequences

- A grant of a container engine's socket is, by the words the Notice says, about the same as no
  sandbox for that chat. The person chooses it knowing that, as they choose Start without the
  sandbox today, but scoped to the one socket and revocable in Settings.
- The Allowed list in Settings shows socket and service grants beside hosts and folders, each
  with Remove.
