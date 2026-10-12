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
- **Where a grant is kept** (D-sock-2). For one chat, in memory, as a host's is. For this
  project on this machine, in `app/sandbox.json` (`sockets_mine`, `services_mine`), which no
  sandboxed chat writes. **Never in a committed file**: a repository could grant itself a socket
  that way. So no "everyone in the project" level is offered.
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
- **Per harness** (D-sock-7). Claude Code takes a socket in `network.allowUnixSockets` (macOS
  only: on Linux its seccomp filter cannot tell sockets apart, so a sandboxed Claude Code chat
  there reaches none), and of every system service only the certificate check, as
  `enableWeakerNetworkIsolation`; any other service's Notice offers Start without the sandbox
  and says why. purlis's own wrap (Codex, opencode) takes a socket as a connect-only
  `unix-socket` rule and a service as a `global-name` lookup.
- **Policy** (D-sock-8). An administrator's `socket-grants: false` forbids every socket and
  service grant and takes away any made, the certificate check included, the committed
  `certificate-checks` too.

## What stays as it was

- **`[sandbox].certificate-checks`** still turns the certificate check on for every chat of the
  project, as it did: projects rely on it, and reading it as a declaration only would break
  their Go tools on the day this ships. The Notice's Allow is the per-person way for every other
  project. Whether the committed key should become a declaration only is the operator's call.
- **Brokered Docker**, purlis running `docker` for the chat after checking the command, is a
  safer route than granting Docker's socket, and a later, optional follow-up.

## Consequences

- A grant of a container engine's socket is, by the words the Notice says, about the same as no
  sandbox for that chat. The person chooses it knowing that, as they choose Start without the
  sandbox today, but scoped to the one socket and revocable in Settings.
- The Allowed list in Settings shows socket and service grants beside hosts and folders, each
  with Remove.
