# Amended 2026-11-19: a chat on a locked repo holds no path to the person's forge login

*Slice 1 of #7133 (#7205); design `docs/research/forge-credential-lock.md` §6.1.* The merge and
release lock is the forge's own rule, with the person's repository-admin role as its one bypass.
A chat that holds the person's own login is that bypass, and merges, tags and releases as them.
Section 5's class 6 (as amended 2026-11-16) holds the person's credential **files**, at the full
sandbox, and at the floor only where the policy enforces it, so under the shipped `watch` floor the
person's `gh` and git over https keep working in a chat, on purpose. That is no longer true of a
chat on a repo that claims the lock.

**A chat claims the lock** where its sandbox holds the credentials class, or one of its tools
mints it a token (`forgelock::claimed`). Slice 3 (#7207) will read the forge's own rulesets for
the claim; until then these two are what says the person asked for their login to be kept away.
For such a chat, what the environment and the config purlis compiles for it hold is not the
file class alone but **the forge credential class**: the files of section 5 and each route below.

| Route | How it is closed |
|---|---|
| `gh` with no minted token | `GH_CONFIG_DIR` is an empty folder in the chat's own temp directory, so `gh` finds no login in a file. That a chat with no token is *told* to mint one, and stops, is the shim's work (#7218, not in this slice): until it lands, `gh` says its own "not logged in". |
| The real `gh` by its absolute path | The same `gh` reads that empty folder: no login in a file. |
| git over https | `GIT_CONFIG_GLOBAL` is a file purlis writes: it includes the person's own `~/.gitconfig` (names, signing), then empties `credential.helper`. `GIT_CONFIG_SYSTEM` is nothing, so no system helper (Apple's `osxkeychain`) is read. `GIT_CONFIG_COUNT`/`KEY_n`/`VALUE_n` empties `credential.helper` once more at the command scope, which outranks a repository's own `.git/config` (after any entries the chat already has). `GIT_TERMINAL_PROMPT=0`. purlis's own helper stays out of a chat's config, as ADR 0070 (2026-10-12) says; a chat's token reaches git through the mint (#7196, #7210). |
| ssh agents | `SSH_AUTH_SOCK` is empty and `GIT_SSH_COMMAND` is `ssh -F /dev/null -o IdentityAgent=none -o IdentitiesOnly=yes` (after purlis's own route's ssh, where the chat has one). |
| The purlis browser | `browser_login` to a forge host, or a host a clone of the project has a remote on, is refused for a chat that claims the lock. |
| A harness's forge connector | By name (`forgelock::CONNECTORS`). Claude Code: a deny rule `mcp__<name>` for each, so every other server, purlis's own and the person's, still loads (no flag turns off one server; `--strict-mcp-config` drops all). Codex: `mcp_servers.<name>.enabled=false` for each name its config files define, and for no other (a name with no transport fails Codex's config, `invalid transport`, measured on 0.169.0). opencode: the session config's `enabled: false` for each (accepted by its schema, measured on 1.19.5). A connector under another name is a residual. |
| **gh's macOS keychain item** | **Not closed.** gh reads it through the system's keychain service, which no variable turns off. Denying that service in the Seatbelt profile is not verified to leave the harness signed in (Claude Code keeps its own login in the keychain), so it is not shipped. |
| **git's own `osxkeychain`** | **Not closed.** `git -c credential.helper=osxkeychain …` outranks every config file and reads the keychain. |
| **A granted agent socket** | **Not closed.** Design §6.1 has a socket granted by `ssh-agent.persons` refused on a locked repo; that refusal is not built. |
| **A bare `ssh`** (Codex, opencode) | **Not closed.** purlis's ssh route includes the person's `~/.ssh/config` (`Include ~/.ssh/config`), so its `IdentityAgent` applies to an `ssh` the chat runs itself. |
| **Claude Code's own ssh** | **Not closed.** Its line has no `GIT_SSH_COMMAND` (its sandbox sets one per command), so `~/.ssh/config`'s `IdentityAgent` applies. The probe reaches this one, and it leaks. |
| **A claim by a mint alone** | **Not closed.** A chat with a mint and no credentials class gets this environment but no file denial: the person's `gh` config and `~/.ssh` stay readable. |

**A probe per harness, and a status that fails closed.** `forgelock::probe` tries each route that
the environment closes in the environment a chat of each harness is composed with, against a
person made for the test, and passes when none returns their identity
(`a_chat_with_no_mint_has_no_forge_credential`). The result is recorded with the harness
version (`forgelock::PROBED`), and a chat is `Locked` only on a probed version with no route open
(`forgelock::status`): an unprobed version, an unknown one, and any with a route open are
`NotLocked`. The open routes are recorded per harness, not one list for all, and a test holds that
a probed route leaked exactly where it is recorded open. Recorded 2026-11-19: Claude Code 2.1.328,
Codex 0.169.0 and opencode 1.19.5, each with routes open (above), so none is locked yet.

**What this does not claim.** A credential the person keeps outside the paths and variables
above (a token in a dotfile, a different keychain, a key outside `~/.ssh`), and a connector purlis
cannot name, are residuals: the status says the chat is locked for the routes purlis knows. A
Claude Code chat's own sandbox sets `GIT_SSH_COMMAND` for each command it runs, through its SOCKS
route, so the lock names none for it (recorded open above); its keys stay held by the file class where it enforces.
