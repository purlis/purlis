# Amended 2026-10-13 (operator ruling R1): certificate checks are on by default

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
