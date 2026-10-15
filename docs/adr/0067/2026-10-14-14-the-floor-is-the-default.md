# Amended (2026-10-14, #2071; the operator's ruling SO-9/SO-10): the floor is the default

Ruled by the operator on 2026-10-14 (#2071, as amended that day). A chat has one of three
levels: the **full sandbox** of this ADR, **the floor**, or **no sandbox**. A project that has
not turned the sandbox on (no `[sandbox] mode`) runs every chat at the floor, where before it ran
them with no sandbox; `mode = "on"` keeps the full sandbox, unchanged and fail-closed.

- **What the floor lets a chat do.** Reach every host, through the chat's own proxy, whose reach
  carries every host and asks about none; this machine's own addresses, a link-local address, a
  metadata service and a host policy pins as never allowed stay refused there. Write every
  folder but what it holds.
- **What it holds.**
  - Every denial class of section 5, path for path, and the keychain's files.
  - The later-code names at every depth anywhere, each letter in either case. Besides the
    project-level names of section 5, the person's own: git's config (`.gitconfig`,
    `.config/git`); ssh's config, `config.d`, `authorized_keys`, `rc` and `environment` (its
    `known_hosts` stays writable) and the `.ssh` folder itself; `Library/LaunchAgents`; the
    shell startup files, fish's config and the logout files; `.claude.json`, `.claude`'s
    hooks and plugins, `.codex`, `.config/opencode`; VS Code's and Cursor's user folders,
    JetBrains', `.cursor` and `.config/zed`.
  - Folders a person runs programs from later: each folder on the person's `PATH`, and the
    package each link there leads to; Homebrew's whole prefix where it is there
    (`/opt/homebrew`, or `Cellar`, `opt`, `lib` and `Homebrew` under `/usr/local`); npm's
    global packages; the person's own Python site folders (`Library/Python`,
    `.local/lib`, whole); the program the chat starts, with its whole package; and each
    declared tool's program. A write to one of these is a sandbox block whose Allow lets that
    chat write the folder (SO-10).
  - Every project's purlis files, not only the chat's own: the state folder under every
    spelling and the manifests under every name, wherever they are and however a name is
    spelled. The full sandbox writes none of these in the chat's own project; Claude Code's
    held names cannot be lifted inside, so at the floor its own project's nested manifests are
    held too (D-391-8). At every level, the vault files of every project are unreadable.
  - Codex and opencode: no Apple Event, nothing opened through Launch Services, no job
    created in launchd, and no `crontab` or `at` run; no unix socket but the hook socket and
    the ones a person allowed (ruling Q1), and the hook socket's folder never written; no port
    of this machine but the chat's proxy's.
  - The certificate check as the project says it, and as policy leaves it.
- **What it does not hold.** What a chat may read outside the denial classes, what a
  program it runs does with the network, and code a later program loads from a place this list
  does not name.
- **Per harness.** Claude Code: its own sandbox, every folder but the held ones writable, no
  domain list, its commands' network through the chat's proxy, local binding off. Codex and
  opencode on macOS: purlis's own profile from `(allow default)` with the floor's denials. A
  brokered `secret exec` for a floor chat runs under the floor's profile, through the proxy.
- **Where it cannot be applied** (ruling Q2): a harness purlis cannot wrap on this system, no
  backend, or a vault registry purlis cannot read. The chat starts as it did before the floor,
  and its tab's sandbox mark says why. A person may still start a chat without the floor,
  audited as the opt-out is.
- **A name in another case.** Measured on the macOS runner (`tools/seatbelt-case.sh`, a step
  of the macOS scenario job, run on every pull request), with a control that runs each
  operation under no rule at all: for a name **already on disk**, macOS's sandbox matches the
  path as it is on disk, not as typed. A read through `.PURLIS/VAULTS` and a write through
  `library/launchagents` are refused by a rule that spells them `.purlis/vaults` and
  `Library/LaunchAgents`, which the step asserts. A held name **not yet on disk** (a new `.ZSHRC`, `LAUNCHAGENTS` or
  `PURLIS.TOML`) is held by the case classes: every held name is written as typed and with a
  class of both cases for each letter, in Claude Code's rules and in the floor's Seatbelt
  regexes, and the step asserts that the classed rules refuse every one of these spellings.
  Whether Claude Code's Linux sandbox honours a bracket class is not measured on Linux; Linux
  compares names as written.
- **No folder to confine to.** The floor confines a chat to no folder, so a chat with no folder
  of its own, one outside the project, or one reached through a link is not refused for it; the
  denials of the folder it stands in hold as at every level. The full sandbox still refuses all
  three.
- **A program that does not answer as its harness is refused, not started without the floor.**
  The floor asks a Claude Code profile's program `--version` as the full sandbox does (V87g).
  One that answers as something else, or not at all, is refused with a sentence that names the
  program, what it answered, and how to fix it. One that does not answer in time is asked once
  more, with twice the wait, before it is refused. Only a system or backend where the floor
  cannot be applied at all starts a chat without it.
