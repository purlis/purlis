# The #6970 inventory, row by row, by family

Every decision purlis makes about a chat, as the #6970 inventory lists it (1,199 rows, read
from `main` at `eb9c63fe8`), with the one family of `[policy]` that switches it (rulings Q308,
Q308a and Q308b; [`docs/policy.md`](policy.md); [ADR
0125](adr/0125-blocks-are-off-by-default-and-opt-in-per-family.md)). Ruling Q308b: "every
inventory row maps to a family". The inventory itself, with each row's default, who changes it
today and which harness it applies to, is on the never-merged branch
`policy-map-research-what-purlis-decides-7cnd8vp8` (`docs/research/policy-inventory.md`); this
table keeps its section, row number, decision and place, and adds the family.

`crates/purlis-core/tests/it/every_inventory_row_maps_to_exactly_one_family.rs` fails where a
row names no family, two, or one purlis does not know, or where a section's count moves.

## How to read it

- **A family** is one of the keys of `[policy]` (`docs/plane-format.md`, `[policy]`).
- **`fixed`** marks what Q308b keeps outside the policy, enforced in every mode, `open`
  included:
  - **the kill switch** (the person's emergency stop);
  - **the grader bundles**: a skill's grade and an outcome eval run in a bundle a skill cannot
    loosen (a skill cannot grade itself);
  - **a chat changing its own policy**: the settings tables a chat's write or commit may not
    change, the state folder that holds grants and nevers, the persona gate's refusal of the
    control surface, the admin lock file and its folder, and the admin lock layer itself, its
    dispatch ceilings included (a lock is judged after the mode and always enforces);
  - **a chat answering for the person**: an ask only the window answers, and a `purlis`
    command's confirmation in the window;
  - **the person's own rules**: their ask and deny rules under every spelling (A7c), as their
    dispatch nevers and Keep blocked hosts hold (those rows are `dispatch`'s and `asks`'s,
    whose wiring keeps them in every mode).
- **A row is the family's even where no choke point asks the family yet.** Such a row stays
  enforced, as before Q308: a family switch never loosens a decision nobody wired. The next
  section names the choke points that do ask.
- **What protects a credential stays with the credential families.** A row that would hand a
  chat a token, a key or a vault's value is `guards.credential`, `secrets` or `vaults`, even
  where it sits in another section (a forge's sign-in, a persona tool's forge subcommands run
  with a vault's value), so no family Q308b added can release one.

## Where each family Q308b added is switched

Each family switches at the choke points below: `off`, the check does not run; `log`, it runs,
lets the call through and writes one line (rule, family, chat, host) to the chat's enforcement
log; `enforce`, it refuses as before. Every other row of the family stays enforced until a choke
point asks it.

| Family | Choke point | Rule in the log | What stays enforced in every mode |
|---|---|---|---|
| `guards.shape` | The Bash guard's arms: caps (`toolgate::Gate::mode`), A7 handoff, A7b consent spelling, A7d sub-agent dispatch, A7e dispatch text, A10 rider (`toolgate::Arm::family`) | the arm's id (`caps`, `A7`, …) | The caps run at the strictest family the gate asks, with `guards.merge` and `guards.git`: a command too big or too deep to read is never shown clear of an enforced arm. A7c (the person's own ask and deny rules) is `fixed`: where the person's rules name a program, a command too big or too deep to read is refused in every mode. The guard's own failure handling (a crash, a timeout, an unknown hook word refuses) is plumbing, not a refusal of the chat's. |
| `guards.plane` | The Bash guard's plane-root arms A3 (branch move) and A3b (destroying reset) | `A3`, `A3b` | Brokered writes, proposals and removal are refused by the sandbox where they matter, and a write to a manifest is `fixed`. |
| `guards.forge` | `chattools::forge_milestones`: a repo named that this workspace does not hold is read from the project's other workspace that holds it | `forge.milestones.workspace` | A forge sign-in, a minted token and a persona tool's forge subcommands run with a vault's value are `guards.credential` and `vaults` rows; the CLI route's file refusal has no plane to ask. |
| `mode` | `dispatchunattended::bypass_refusal_in` and `start_of_a_persona_chat_in` (the app's `handoff.rs`): a chat one chat starts for another on a profile whose command switches the prompts off | `dispatch.prompts-off` | A task's level is still capped at auto: a chat another chat asked for starts without its profile's words that switch the prompts off (`start::ready_in`), and its tab says so. A grade's chat is `fixed`. |
| `tools` | `personaverbs::chatstart::unenforced` and `personaprofile::named_by`: a persona whose `disallowed-tools:` the harness cannot carry | `persona.disallowed-tools` | The deny-list itself is written into Claude Code's settings in every mode. |
| `trust` | `wiring::not_asked` and `personaprofile::offers_of`: a profile's command, or the harness declaration it runs on, nobody approved on this machine | `profile.approval`, `declaration.approval` | Every template but `open` enforces it (Q329). A profile in a file purlis refused (a local file git would commit). Where it is lifted, the chat's tab says it started unapproved (`wiring::unapproved_notice`). Extensions are approved against the machine's config, which no plane's policy reaches. |
| `runners` | `runnerwrite::held_for_the_person`, asked by the app before it holds a runner chat's dispatch (`runnerdispatches::held_from_a_pool`) or write (`brokered.rs`) | `runner.dispatch`, `runner.write` | No vault value, token or sign-in goes to a runner that is not the person's own (a `vaults` row); a runner's taint. |
| `consents` | `windowonly::asks`: a window-only refusal (Trust this clone) is raised as an ask | `window-only.ask` | The press itself stays the window's: each releases a credential, publishes, or sends data off the machine, and which a chat may make needs a ruling of its own, as `skills` does. |
| `limits` | `dispatchdecision::asked_by_a_chat` (depth, running per chat, live per lineage, may-dispatch, may-run-at-once, tokens per session: the dispatch is decided again with `Limits::lifted`), and `gitbroker::commit::paced` (30 commits in 10 minutes) | `dispatch.<limit>`, `commit.rate` | An administrator's ceiling, whichever level's value it bounds (a lifted limit goes up to it), depth beyond 8 (`DEEPEST`), and a 0 that switches dispatch off (the project's word), the destination workspace's included. |

## Rows per family

| Family | Rows |
|---|---:|
| `sandbox` | 98 |
| `network` | 58 |
| `ports` | 32 |
| `processes` | 47 |
| `guards.git` | 55 |
| `guards.merge` | 32 |
| `guards.release` | 34 |
| `guards.save` | 58 |
| `guards.credential` | 32 |
| `asks` | 19 |
| `dispatch` | 57 |
| `vaults` | 90 |
| `browser` | 83 |
| `skills` | 69 |
| `secrets` | 34 |
| `guards.forge` | 4 |
| `guards.shape` | 53 |
| `guards.plane` | 33 |
| `mode` | 28 |
| `tools` | 40 |
| `trust` | 32 |
| `runners` | 21 |
| `consents` | 25 |
| `limits` | 115 |
| `fixed` | 50 |
| **Total** | **1,199** |

<!-- rows -->
## Sandbox files and paths

| # | Decision | Where | Family |
|---|---|---|---|
| 1 | Whether a project's chats run at the full sandbox (`[sandbox] mode = "on"`) | `crates/purlis-core/src/sandbox.rs:818` (read), `crates/purlis-core/src/sandbox.rs:398` (`on_block` written by the one-time offer), `app/src-tauri/src/sandboxing.rs:712` (`answer_sandbox_offer`), `crates/purlis-core/src/sandbox/local.rs:661` (`offer_due`) | `sandbox` |
| 2 | A new project is scaffolded with `[sandbox]`, the default egress and no `mode`, so its chats start at the floor | `crates/purlis-core/src/sandbox.rs:405` | `sandbox` |
| 3 | A `charter.toml` that is missing or unreadable means "the sandbox is unknown", so the chat is refused (never read as "off") | `crates/purlis-core/src/sandbox.rs:8309` (`PlaneMissing`), `crates/purlis-core/src/sandbox.rs:435` (`Plane::read`); `crates/purlis-core/src/sandbox/personal.rs:327` reads it as `On` | `sandbox` |
| 4 | Most of `charter.toml` read at a start is 1 MiB. Links, FIFOs and devices are refused | `crates/purlis-core/src/sandbox.rs:563` | `sandbox` |
| 5 | An unknown or ill-typed key in `[sandbox]` is refused and read as the narrow value (bad `mode` means on, bad `certificate-checks` means off, bad `own-terminal-only` means on, bad `policy` means never) | `crates/purlis-core/src/sandbox.rs:834`, `crates/purlis-core/src/sandbox.rs:818-833`, `crates/purlis-core/src/sandbox/team.rs:61` | `sandbox` |
| 6 | Team's say: `[sandbox] policy = "local"` (each person picks) or `"never"` (nobody below full; opt-out refused; `mode="off"` refused) | `crates/purlis-core/src/sandbox/team.rs:61`, `crates/purlis-core/src/sandbox/policy.rs:493` (`team_never` feeds `forbids_opt_out`) | `sandbox` |
| 7 | Team's off (`mode = "off"` under `local`) reaches a person only when they press Accept, and only when it is committed. The commit and author are named only when verified on the fetched upstream | `crates/purlis-core/src/sandbox/team.rs:175` (`team_off`), `crates/purlis-core/src/sandbox/team.rs:526` (`answer`), `crates/purlis-core/src/sandbox/local.rs:1175`, `app/src-tauri/src/sandboxing.rs:678` | `sandbox` |
| 8 | The person's own level (`on`/`floor`/`off`) per project and per persona (the persona's wins), kept in `purlis.local.toml`. A level is honoured only when the person set or confirmed it in Settings. A lower level the file alone says waits for Confirm | `crates/purlis-core/src/sandbox/personal.rs:313` (`in_force`), `:340` (`confirm`), `:362` (`set_by_you`); `app/src-tauri/src/yoursandbox.rs` | `sandbox` |
| 9 | Bound on the person's level: full only under team `never` or admin `opt-out:false`; default full where the project turned it on, the floor otherwise, off once the team's off is accepted | `crates/purlis-core/src/sandbox/team.rs:616` (`allowed_levels_under`), `:585` (`clamp`); `crates/purlis-core/src/sandbox.rs:8195` (`bounded`) | `sandbox` |
| 10 | A chat another chat asked for (dispatch/handoff) runs at the strictest level across the chain of chats that asked. A start that does not say how it was made, or whose chain is unknown, never takes `off` | `crates/purlis-core/src/sandbox.rs:8141` (`your_level`), `:8162` | `sandbox` |
| 11 | Per-chat "Start without the sandbox": made in the window only (no CLI, file, profile or chat), lasts one run, never inherited, audited (`trust.sandbox.off`). The reason is kept to 200 characters | `crates/purlis-core/src/sandbox.rs:7804` (`OptOut`), `:7811` (`MOST_REASON_CHARS`), `:8284` | `sandbox` |
| 12 | No opt-out on a sandbox-only profile, nor on a runner | `crates/purlis-core/src/start.rs:872` (`profile_refusal`), `crates/purlis-core/src/start.rs:832` (`without_sandbox_lock`), `app/src-tauri/src/hostedsessions.rs:4951` (`SANDBOXED_ON_A_RUNNER`) | `sandbox` |
| 13 | A system with no backend (Windows): chats start unsandboxed with charter as the actor. Under a lock that requires the sandbox they are refused (`RequiredWithoutBackend`) | `crates/purlis-core/src/sandbox.rs:8304`, `:8299`; `crates/purlis-core/src/sandbox.rs:1574` (`has_backend`) | `sandbox` |
| 14 | Your `on` means the full sandbox or no start (never a floorless start) | `crates/purlis-core/src/sandbox.rs:8256` | `sandbox` |
| 15 | Where the floor cannot be applied in a project that has not turned the sandbox on, the chat starts with no sandbox and says so (`Floorless`) rather than being refused | `crates/purlis-core/src/sandbox.rs:8262-8278` | `sandbox` |
| 16 | An admin policy that requires the sandbox runs every project's chats as if `mode="on"` (its `[sandbox]` as written, or the default presets) | `crates/purlis-core/src/sandbox.rs:785` (`Said::in_force`) | `sandbox` |
| 17 | Which harnesses can be sandboxed: Claude Code (its own sandbox via `--settings`, or the wrap on Linux/runner), Codex and opencode (wrap). Any other harness has no compiler (`NoCompiler`), and a held-back harness is refused | `crates/purlis-core/src/sandbox.rs:6291` (`compiler`), `:6297` (`compiler_on`), `:6308` (`never_on`); `crates/purlis-core/src/harness/claude.rs:393`, `crates/purlis-core/src/harness/codex.rs:385`, `crates/purlis-core/src/harness/opencode.rs:229`; `crates/purlis-core/src/sandbox.rs:1593,1653` (`purlis_wraps`) | `sandbox` |
| 18 | A chat in a sandboxed project (full or floor, opted out or not) never runs over ACP (level 3). It starts in its terminal | `crates/purlis-core/src/acp/launch.rs:90`, `crates/purlis-core/src/acp.rs:12` | `sandbox` |
| 19 | A sandboxed Codex chat (and a sandboxed Claude Code chat on Linux) starts only on a sandbox-only profile, so its home is never one that an unsandboxed run uses | `crates/purlis-core/src/start.rs:901-917`, `crates/purlis-core/src/start.rs:929` (`wants_a_sandbox_only_profile`) | `sandbox` |
| 20 | Choosing "No sandbox" at setup first moves the project's plain-file vaults into the keychain. It is refused where the team's committed `vaults.json` registers a plain-file vault | `crates/purlis-core/src/sandbox/personal/no_sandbox.rs:42` (`TeamVault`), `:62` (`report`) | `sandbox` |
| 21 | Full sandbox: a chat writes only its own folder (cwd), its own temp folder, what a person granted and the project's cache home. It reads everything except the denial classes (no read is ever granted) | `crates/purlis-core/src/sandbox/seatbelt.rs:534` (wrap: cwd + tmp), `crates/purlis-core/src/sandbox/claude.rs:471` (Claude: Claude Code's own cwd-writable default + `allowWrite`), `crates/purlis-core/src/sandbox/grant.rs:37` | `sandbox` |
| 22 | Cross-workspace: what a chat writes follows its cwd. A chat at the plane root writes every workspace (except held paths). A chat in a workspace or branch folder writes only that folder. Reads of other workspaces' files are not limited at any level | `crates/purlis-core/src/sandbox.rs:5248` (`folders_of_a_chat`), `crates/purlis-core/src/sandbox.rs:5212` (`form_in`) | `sandbox` |
| 23 | Floor: every folder is writable except what the denial classes and the floor hold | `crates/purlis-core/src/sandbox/claude.rs:358` (`allowWrite = ["/"]`), `crates/purlis-core/src/sandbox/seatbelt.rs:979` (`FLOOR_BASE (allow default)`) | `sandbox` |
| 24 | Floor holds the person's `PATH` folders and the packages their links lead to, Homebrew's prefixes, `~/Library/Python`, `~/.local/lib` and npm globals (none that holds or is inside the project, or is the home or above it). It follows at most 4096 links per folder and caches the result for 5 s | `crates/purlis-core/src/sandbox.rs:3748` (`FLOOR_HOLDS_PATH = true`), `:3756`, `:3870` (`PREFIXES_HELD`), `:3879`, `:3777` | `sandbox` |
| 25 | Floor holds the harness's own program and each declared tool's program, plus what purlis arms a chat with (its binary, plugin, git hooks, skill sets). At every level the chat is refused if one of these lies where it writes (`KitWritable`) | `crates/purlis-core/src/sandbox.rs:4863` (`hold_programs`), `:4783` (`hold_kit`), `:4755` (`hold_wrap`) | `sandbox` |
| 26 | Floor: purlis makes `~/.local/share`, `~/.config`, the XDG homes and the folders above the liftable names before the start (they are pinned, so the chat could not make them) | `crates/purlis-core/src/sandbox.rs:7589` (`usual_home_folders`), `crates/purlis-core/src/sandbox/claude.rs:146` | `sandbox` |
| 27 | Full: the person's `~/.claude/settings.json` and `settings.local.json` are never written by a chat (commands or the Edit tool) | `crates/purlis-core/src/sandbox/claude.rs:341` | `sandbox` |
| 28 | Outcome-eval shell: no temp folder but its own (Claude's `/tmp/claude` is denied), and no hosts, sockets or Allow at any scope | `crates/purlis-core/src/sandbox/policy.rs:1243` (`holding_to_its_folder`), `crates/purlis-core/src/sandbox/claude.rs:462`, `:119` (`SHARED_TEMP`) | `fixed` |
| 29 | A grade's chat keeps no Allow of any scope | `crates/purlis-core/src/sandbox/policy.rs:1259` (`holding_no_kept_grants`) | `fixed` |
| 30 | Each vault provider's storage and local session, as the registry resolves them, under every spelling of the state folder | `crates/purlis-core/src/sandbox.rs:2172-2199`, `:2277-2291` | `sandbox` |
| 31 | Every project's `vaults/`, `fingerprint.key` and `vault-links.json` anywhere on disk, any case (cross-project) | `crates/purlis-core/src/sandbox/claude.rs:199`, `crates/purlis-core/src/sandbox/seatbelt.rs:831` (`vaults_everywhere_rules`), `:837` (no exec from them) | `sandbox` |
| 32 | `~/.config/op` (or `$OP_CONFIG_DIR`), `~/.op`, `~/.vault-token` | `crates/purlis-core/src/sandbox.rs:2315-2317` | `sandbox` |
| 33 | On a runner: gh's and glab's config folders (incl. `GH_CONFIG_DIR`, `GLAB_CONFIG_DIR`) | `crates/purlis-core/src/sandbox.rs:2320-2343` | `sandbox` |
| 34 | Vault registries (shared and local): readable, never written | `crates/purlis-core/src/sandbox.rs:2348-2349` | `sandbox` |
| 35 | A vault file's other names (hard links) inside the project. It looks through up to 200k entries and holds up to 20k names, skipping `.git`, `target` and `node_modules`. A list that cannot be told whole refuses the chat (`KeptUntold`) | `crates/purlis-core/src/sandbox.rs:2734`, `:3158-3169` | `sandbox` |
| 36 | macOS `~/Library/Keychains` folder. Keychain services rest on each item's own ACL | `crates/purlis-core/src/sandbox/seatbelt.rs:470` (`keychains`) | `sandbox` |
| 37 | `<state>/app` (every spelling) | `crates/purlis-core/src/sandbox.rs:2356` | `sandbox` |
| 38 | Theme drafts, skill-delete keeps, chat settings files, approvals (profile command, harness declaration, persona MCP) | `crates/purlis-core/src/sandbox.rs:2359`, `:2368`, `:2439`, `:2431`; `$PURLIS_HOME` copies `:2450-2470` | `sandbox` |
| 39 | Every chat's hook spool and keys; dispatch records; dispatched chats' kept screens; closed chats' records; `arrived` state; browser timelines (another persona's chats are never read) | `crates/purlis-core/src/sandbox.rs:2373`, `:2391`, `:2398`, `:2405`, `:2412`, `:2416` | `sandbox` |
| 40 | Handbacks (`<state>/handbacks`): never written. Also never read where the harness's hooks run outside its sandbox (Claude Code on its own sandbox) | `crates/purlis-core/src/sandbox.rs:2382`, `:1797` (`and_what_waits_for_a_chat`), `:7455` | `sandbox` |
| 41 | Making a state folder that is not there; the state folder entry itself (never moved or renamed); Skill studio records | `crates/purlis-core/src/sandbox.rs:2480`, `:2491`, `:2518` | `sandbox` |
| 42 | The app state in the data home (`<data>/app-state`, plus the data home entry) | `crates/purlis-core/src/sandbox.rs:1373` (`app_state_held`), `:2721` | `sandbox` |
| 43 | The folders chats' servers listen in (doors) | `crates/purlis-core/src/sandbox.rs:2714` | `sandbox` |
| 44 | purlis's config home, every spelling (approvals, grants, confirmed levels, `sandbox-grants/`) | `crates/purlis-core/src/sandbox.rs:2548` | `sandbox` |
| 45 | Human client scopes (daemon folder); runner peers, pinned runners, left runners, runner hours, cluster kubeconfig copies, Docker-context TLS copies, forge ETag store | `crates/purlis-core/src/sandbox.rs:2565`, `:2571-2611` | `sandbox` |
| 46 | Files each Docker-TLS pool names (CA, client cert and key), by name and real path | `crates/purlis-core/src/sandbox.rs:2622-2624` | `sandbox` |
| 47 | This machine's profiles folder; session-host browser data; Docker settings copies; SSH-forward ends; runner login waits; browser logins | `crates/purlis-core/src/sandbox.rs:2633`, `:2640`, `:2648`, `:2655`, `:2663`, `:2669` | `sandbox` |
| 48 | Downloaded browser engines; the harness versions a runner installs; on a runner, purlis versions installed over SSH | `crates/purlis-core/src/sandbox.rs:2687`, `:2693`, `:2555` | `sandbox` |
| 49 | The session host's socket folder | `crates/purlis-core/src/sandbox.rs:2707` | `sandbox` |
| 50 | Admin policy folder `/etc/purlis` | `crates/purlis-core/src/sandbox.rs:1826`, `crates/purlis-core/src/sandbox/policy.rs:160` | `fixed` |
| 51 | A cluster runner's link-handover folder `/run/purlis-link` | `crates/purlis-core/src/sandbox.rs:985`, `:2536` | `sandbox` |
| 52 | Every other harness's default sign-in folder (or only its files, where the chat works inside it); the chat's own profile's credential files are kept from its commands only where the harness runs commands apart from itself | `crates/purlis-core/src/sandbox.rs:7525`, `crates/purlis-core/src/sandbox/signin.rs:102`, `:118` | `sandbox` |
| 53 | Pool setup's Get help chat: `~/.kube`, every `KUBECONFIG` file, no hosts, sockets or Allow | `crates/purlis-core/src/sandbox.rs:2755`, `crates/purlis-core/src/sandbox/policy.rs:1276` | `sandbox` |
| 54 | 93 `PLANTED` names never written at any depth: `.git` config, hooks and worktrees; `.husky`/`.githooks`; `.claude/*`, `.mcp.json`, opencode/codex/`.agents` configs; `.vscode`/`.idea`/`.cursor`; shell rc files; `.ssh` config and authorized_keys; `~/.gitconfig`; LaunchAgents; systemd/autostart; editors' user settings | `crates/purlis-core/src/sandbox.rs:1070`; `crates/purlis-core/src/sandbox/claude.rs:240`; `crates/purlis-core/src/sandbox/seatbelt.rs:902-908` (floor) | `sandbox` |
| 55 | 11 `MAY_LIFT` home names (gh, glab, kuberc, fish/nvim/bash-completion/dbus share dirs) can be lifted for one chat at the floor, by exact folder, with no link on the way | `crates/purlis-core/src/sandbox.rs:1199`; `crates/purlis-core/src/sandbox/grant.rs:828` (`lift`) | `sandbox` |
| 56 | Project manifests (`purlis.toml`/`charter.toml`, `purlis.local.toml`/`charter.local.toml`) at the root and in each folder from cwd up to it. At the floor, in every project on disk, with every state folder | `crates/purlis-core/src/sandbox.rs:1260`, `:1282`; `crates/purlis-core/src/sandbox/seatbelt.rs:758` (`every_project_rules`); `crates/purlis-core/src/sandbox/claude.rs:414` | `fixed` |
| 57 | Every `workspaces/<ws>/workspace.json`; no workspace folder made, moved or removed | `crates/purlis-core/src/sandbox.rs:1408`; `crates/purlis-core/src/sandbox/seatbelt.rs:784`, `:812`; `crates/purlis-core/src/sandbox/codex.rs:683`, `crates/purlis-core/src/sandbox/claude_wrap.rs:334` | `sandbox` |
| 58 | The project's harness-declarations folder and repo inventory | `crates/purlis-core/src/sandbox.rs:1843`, `:1852` | `sandbox` |
| 59 | Skills folders of the project, of each workspace and persona (`*` patterns for ones made later), where skills reach chats here, and always on a runner. A chat's write becomes a draft | `crates/purlis-core/src/sandbox.rs:1332` (`skills_held`), `:1866` | `sandbox` |
| 60 | Docker Desktop/OrbStack/Rancher `bin` folders and sockets in the home; Podman's socket in `$TMPDIR`; rootless Docker/Podman sockets in `$XDG_RUNTIME_DIR`; Podman's folders | `crates/purlis-core/src/sandbox.rs:1876-1900`, `:1930-1959`, `:2810`; `crates/purlis-core/src/dockerpool/settings.rs:127,137,141`; `crates/purlis-core/src/dockerpool.rs:2691` | `sandbox` |
| 61 | gh/glab folders and XDG later-code names where the person's environment moves them (absolute, or relative to cwd) | `crates/purlis-core/src/sandbox.rs:2956`, `:2858`, `:5228` (`forge_folders_here`) | `sandbox` |
| 62 | The kubectl and credential-plugin programs of every cluster pool, with their whole package | `crates/purlis-core/src/sandbox.rs:1975`, `:2098` | `sandbox` |
| 63 | What protected configs name elsewhere (`core.hooksPath`, scripts a harness config runs), resolved at start. The search reaches depth 4 and 20k entries (clones depth 6). More than 64 moves or 1024 names refuses the chat (`Unread`) | `crates/purlis-core/src/sandbox/planted.rs:18`, `:21`, `:29`, `:749`, `:752`; `crates/purlis-core/src/sandbox.rs:1985-2006` | `sandbox` |
| 64 | The project root's own `.git`, and in a purlis branch folder its clone's git folder: only the app commits, saves, updates and pushes there | `crates/purlis-core/src/sandbox.rs:1733` (`roots_git_held`), `:1690` (`clones_git_held`), `:1750` | `sandbox` |
| 65 | Claude Code (own sandbox): each repo's git folder present at start is pinned whole. A path holding a glob character refuses the chat (`GitFolderUnheld`). More than 50k repositories or entries refuses it (`RepositoriesUnfound`). No Allow is offered for a pinned git folder | `crates/purlis-core/src/sandbox.rs:7484-7500`, `:1767`; `crates/purlis-core/src/sandbox/planted.rs:252-262`; `crates/purlis-core/src/sandbox/grant.rs:534`, `:544` | `sandbox` |
| 66 | Later-code names that appear another way (not by a chat's write) are told on a Notice, never refused or removed. Bounds: 500k entries, 20k names, 30 s | `crates/purlis-core/src/sandbox/arrived.rs:72-129` | `sandbox` |
| 67 | A sandboxed chat's folder must be a real directory inside the project, with no link from the plane down | `crates/purlis-core/src/sandbox.rs:6434` (`folder_refusal`) | `sandbox` |
| 68 | A denial that covers `/`, the home, the project or the chat's own folder refuses the chat (rather than starting it read-only) | `crates/purlis-core/src/sandbox.rs:6496` (`covering`), `:6481` | `sandbox` |
| 69 | A wrapped chat whose folder holds (or is inside) the harness's own home is not wrapped | `crates/purlis-core/src/sandbox/codex.rs:607`, `crates/purlis-core/src/sandbox/opencode.rs:229`, `crates/purlis-core/src/sandbox/claude_wrap.rs:178` | `sandbox` |
| 70 | A folder grant only from an allowlist: inside the project, or a folder the person listed in Settings › Sandbox (this machine), resolved when listed | `crates/purlis-core/src/sandbox/grant.rs:633` (`write`), `crates/purlis-core/src/sandbox/local.rs:1618`, `:1628`; `app/src-tauri/src/sandboxing.rs:3523`, `:3569`, `:3613` | `sandbox` |
| 71 | Never granted: `/`; a folder holding the home or the project; `PATH` folders (except at the floor); every harness's home and temp; system temp `claude-*` roots; the state folder; any class path; a manifest or planted name; a name with a backtick or an invisible character | `crates/purlis-core/src/sandbox/grant.rs:680-790`, `:368-420` (`SYSTEM_TEMP`, `HARNESS_TEMP`, `harness_homes`), `:429` (`Ground::of`) | `sandbox` |
| 72 | Folder grant scopes: this chat (in memory) or "you" (this project on this machine, in the config home `sandbox-grants/<clone>.json`). Never once, never project, never every project | `app/src-tauri/src/sandboxing.rs:1722-1733`, `crates/purlis-core/src/sandbox/policy.rs:592` (machine scope), `crates/purlis-core/src/sandbox/grant.rs:137`, `crates/purlis-core/src/sandbox/local.rs:41`, `:1229` | `sandbox` |
| 73 | Admin `write-grants:false` forbids every folder grant. `allow-scopes` limits the scopes an Allow may keep | `crates/purlis-core/src/sandbox/policy.rs:517`, `:625`, `:592` | `sandbox` |
| 74 | The old project-local `<state>/app/sandbox.json` is never read (a repo could carry grants) | `crates/purlis-core/src/sandbox/local.rs:32` | `sandbox` |
| 75 | Package caches: only with the `toolchains` preset, a cache home of the project's own under the data home (never the person's caches), with 10 tools' env vars | `crates/purlis-core/src/sandbox.rs:248` (`widens_caches`), `:4028`; `crates/purlis-core/src/sandbox/caches.rs:65`, `:339`, `:350` | `sandbox` |
| 76 | In the cache home, writes only to cargo's registry, git checkouts and bare-repo insides plus lock files. Never cargo's config, credentials or `bin`, a bare repo's `config`/`hooks`, or a folder purlis made | `crates/purlis-core/src/sandbox/caches.rs:112-131`; `crates/purlis-core/src/sandbox/claude.rs:289-330` | `sandbox` |
| 77 | Cargo and Gradle config in the cache home are seeded from the person's, keeping only registry, source and network keys (no token, credential provider, alias, proxy or JVM option) | `crates/purlis-core/src/sandbox/caches.rs:467`, `:578`, `:631`, `:657` | `sandbox` |
| 78 | Codex home of a sandboxed chat = the sandbox-only profile's folder. It writes only `HOME_DIRS`, `HOME_FILES` and `SHARED_STORES` (sessions, history, auth, memories, goals, queue), never config, skills or plugins. Codex's own `config.toml` is written by purlis | `crates/purlis-core/src/sandbox/codex.rs:116-139`, `:213`, `:238` | `sandbox` |
| 79 | opencode writes only its data `log`/`storage` and sessions DB; never `auth.json`/`mcp-auth.json`; config and cache are read-only; snapshots off; state goes to the chat's own `XDG_STATE_HOME` | `crates/purlis-core/src/sandbox/opencode.rs:106`, `:109`, `:225` | `sandbox` |
| 80 | A wrapped Claude Code writes only the turn's `HOME_DIRS`/`HOME_FILES` in its config folder; never settings, hooks, skills, plugins or `.claude.json` | `crates/purlis-core/src/sandbox/claude_wrap.rs:96`, `:117`, `:125` | `sandbox` |
| 81 | A sandboxed Claude Code chat loads its project's `.claude/settings*.json` only if they hold just `$schema`, `permissions` (deny/ask), `enabledPlugins`, `model`, `outputStyle` or `theme`. Otherwise it runs with `--setting-sources user`, and eval/judge chats load none | `crates/purlis-core/src/harness/claude/project.rs:31`, `:41`; `crates/purlis-core/src/harness/claude.rs:100` | `sandbox` |

## Network and egress

| # | Decision | Where | Family |
|---|---|---|---|
| N1 | Whether a chat's network is held at all (a sandbox is in force: project `[sandbox]` turned on, or a policy that requires it) | `crates/purlis-core/src/sandbox.rs:517` (`Plane::in_force`), `crates/purlis-core/src/sandbox.rs:785` (`Said::in_force`) | `network` |
| N2 | At the Basic sandbox (the floor) a chat reaches **every** host through its own proxy, with nothing asked, except this machine's own addresses and policy's `never-hosts` | `crates/purlis-core/src/sandbox.rs:4355` (`reach::Reach::anything()`), `crates/purlis-core/src/sandbox/reach.rs:242`; Claude settings `crates/purlis-core/src/sandbox/claude.rs:529` (`floor`, no `allowedDomains`) | `network` |
| N3 | At the full sandbox a chat reaches only listed hosts: the presets in force, the project's hosts, your hosts, the persona's hosts, and Allows | `crates/purlis-core/src/sandbox/reach.rs:344` (`Reach::decide`), `crates/purlis-core/src/sandbox.rs:3435` (`hosts`), `crates/purlis-core/src/sandbox.rs:3498` (`granted_hosts`) | `network` |
| N4 | The three egress presets and what each lists (`model-providers`, `forge`, `toolchains`) | `crates/purlis-core/src/sandbox.rs:254` (`Preset::own_hosts`), forge list `crates/purlis-core/src/sandbox.rs:36`, GitHub Actions log/artifact storage accounts `crates/purlis-core/src/sandbox.rs:59` | `network` |
| N5 | Which presets a project turns on | `crates/purlis-core/src/sandbox.rs:848` (`egress` read), default `crates/purlis-core/src/sandbox.rs:226` (`Preset::DEFAULT = ALL`), scaffold `crates/purlis-core/src/sandbox.rs:405`, `crates/purlis-core/src/sandbox.rs:398` | `network` |
| N6 | The `forge` preset also carries each `[[forge]]` host the project names, unless policy refuses it | `crates/purlis-core/src/sandbox.rs:3448` (in `hosts`), `crates/purlis-core/src/sandbox.rs:499` (`forge_hosts`) | `network` |
| N7 | `toolchains` preset also lets a chat write the project's package caches | `crates/purlis-core/src/sandbox.rs:248` (`widens_caches`) | `network` |
| N8 | Project's own hosts: `[sandbox] hosts` in committed `charter.toml`, applied to everyone with no approval (told once per change) | `crates/purlis-core/src/sandbox/hosts.rs:40` (`KEY`), `crates/purlis-core/src/sandbox/hosts.rs:714` (`in_force`), told-once `crates/purlis-core/src/sandbox/local.rs:799` | `network` |
| N9 | Your own hosts: `[sandbox] hosts` in `charter.local.toml`, granted only if git ignores the file and you confirmed each host on this machine | `crates/purlis-core/src/sandbox/hosts.rs:813` (`personal`), `crates/purlis-core/src/sandbox/hosts.rs:828` (`confirmed_only`), record `crates/purlis-core/src/sandbox/local.rs:1041` | `network` |
| N10 | A persona's hosts: `[sandbox.personas.<name>] hosts` in committed `charter.toml`, reach nothing on a machine until the person allows the exact list (digest); any change re-asks | `crates/purlis-core/src/sandbox/persona.rs:77` (`KEY`), `crates/purlis-core/src/sandbox/persona.rs:476` (`in_force_here`), `crates/purlis-core/src/sandbox/persona.rs:343` (`digest`); Allow `app/src-tauri/src/sandboxing.rs:1184` | `network` |
| N11 | Precedence when two levels list a host: Project, then You, then Persona; in reach: Open, Persona, You, Chat | `crates/purlis-core/src/sandbox/hosts.rs:714`, `crates/purlis-core/src/sandbox/reach.rs:38` (`By`) | `network` |
| N12 | What may be written as a host: domain, `*.domain` (never the domain itself, never a whole TLD), IPv4, bracketed IPv6, optional `:port`; never a URL, range, user@, single label, name ending in a number | `crates/purlis-core/src/sandbox/hosts.rs:65` (`Host::parse`), `crates/purlis-core/src/sandbox/hosts.rs:204` (`name_of`) | `network` |
| N13 | A chat never reaches this machine (loopback, 0/8, unspecified, own interface addresses), link-local, multicast, broadcast, or cloud metadata (AWS/GCP/Azure/Alibaba/Oracle IPs), even via a name that resolves there, unless that exact address:port is listed | `crates/purlis-core/src/sandbox/hosts.rs:388` (`refused_address`), `crates/purlis-core/src/sandbox/hosts.rs:437`, metadata `crates/purlis-core/src/sandbox/hosts.rs:321`, `:328`, `:372`; at connect `crates/purlis-core/src/sandbox/egress.rs:1993` (`reachable`); `crates/purlis-core/src/sandbox/reach.rs:352` | `network` |
| N14 | A host that is one of this machine's own interface addresses is dropped from every grant | `crates/purlis-core/src/sandbox/hosts.rs:745` (`off_this_machine`) | `network` |
| N15 | A host nothing lists is **asked** about live (connection held) rather than refused | `crates/purlis-core/src/sandbox/reach.rs:372` (`Decision::Ask`), `crates/purlis-core/src/sandbox/asks.rs:1` | `network` |
| N16 | How long a held connection waits for the person | `crates/purlis-core/src/sandbox/asks.rs:55` (`HOLD` 60 s) | `network` |
| N17 | While the person is Quiet, a held connection waits longer | `crates/purlis-core/src/sandbox/asks.rs:59` (`MOST_HELD_OPEN` 15 min) | `network` |
| N18 | New hosts within a window are grouped into one ask | `crates/purlis-core/src/sandbox/asks.rs:62` (`GROUP` 2 s) | `network` |
| N19 | Most connections held at once; one more is refused unasked | `crates/purlis-core/src/sandbox/asks.rs:65` (`MOST_HELD` 16) | `network` |
| N20 | With nobody to ask (hooks not listening, chat has no number) a connection is refused at once | `crates/purlis-core/src/sandbox/asks.rs:43` (doc), `Answer::NobodyToAsk` | `network` |
| N21 | Scopes an Allow of a host may be kept at: once (= "Allow next time", in memory), this chat, you (this project on this machine, `charter.local.toml`), project (committed `charter.toml`); never "machine" | `crates/purlis-core/src/sandbox/grant.rs:137` (`Level`), `crates/purlis-core/src/sandbox/policy.rs:602` (Machine only for socket/service), host file by level `app/src-tauri/src/sandboxing.rs:1554` (`hosts_file`), once refused for host `app/src-tauri/src/sandboxing.rs:1722` | `network` |
| N22 | "Allow next time" for a host: one next attempt only, in memory, lost on restart | `crates/purlis-core/src/sandbox/asks.rs:24`; command `app/src-tauri/src/network.rs:1122` (`allow_next_time`) | `network` |
| N23 | "Keep blocked": refuses that host now and after, until an Allow | `crates/purlis-core/src/sandbox/asks.rs:467`, record `crates/purlis-core/src/sandboxblock_record.rs:415` | `network` |
| N24 | Hosts policy pins as never allowed are refused and never asked about, at every level | `crates/purlis-core/src/sandbox/policy.rs:540` (`never_allows`), `crates/purlis-core/src/sandbox/reach.rs:369`, `crates/purlis-core/src/sandbox/reach.rs:267` (`never`) | `network` |
| N25 | Policy allowlist of hosts: any level's host not covered is refused | `crates/purlis-core/src/sandbox/policy.rs:566` (`refuses`), `crates/purlis-core/src/sandbox/policy.rs:460` | `network` |
| N26 | Allow decisions are taken from the window alone; a chat (or a remote link) cannot allow a host, port or socket | `crates/session-protocol/src/ui.rs:203` (`WINDOW_ONLY`, incl. `allow_blocked_host`, `allow_next_time`, `allow_sandbox_port`, `allow_sandbox_block`); core re-judges the target `app/src-tauri/src/sandboxing.rs:1687` (`judged`) | `network` |
| N27 | No chat may change any `[sandbox]` key (mode, presets, hosts) through a brokered write | `crates/purlis-core/src/sandbox.rs:7170` (`changes_a_sandbox_key`) | `network` |
| N28 | Network decided by host and port only; no TLS interception, no purlis certificate | `crates/purlis-core/src/sandbox/egress.rs:16`, `crates/purlis-core/src/sandbox/reach.rs:13` | `network` |
| N29 | Proxy carries only `CONNECT` to port 443 and plain absolute-form requests to port 80 for hosts listed without a port; a host listed with a port only on that port | `crates/purlis-core/src/sandbox/egress.rs:92` (`TUNNEL_PORTS`), `crates/purlis-core/src/sandbox/egress.rs:95` (`PLAIN_PORTS`), `crates/purlis-core/src/sandbox/egress.rs:160` (`allows_on`) | `network` |
| N30 | Proxy head rules: strict parse, ≤ 16 KiB head, no `Transfer-Encoding`, one `Host`, `Connection: close`; SOCKS5 CONNECT only, no auth | `crates/purlis-core/src/sandbox/egress.rs:106` (`HEAD_MAX`), `crates/purlis-core/src/sandbox/egress.rs:1822`, `crates/purlis-core/src/sandbox/egress.rs:1524` | `network` |
| N31 | Proxy limits: 64 connections at once, 600 s idle, 30 s for the head | `crates/purlis-core/src/sandbox/egress.rs:131` (`LIMITS`) | `network` |
| N32 | Refusals kept per proxy for brokered `secret exec` and the Block | `crates/purlis-core/src/sandbox/egress.rs:99` (`REFUSALS_KEPT` 8) | `network` |
| N33 | Wrapped chat's traffic pointed at the proxy by env; `NO_PROXY` emptied | `crates/purlis-core/src/sandbox/seatbelt.rs:1062` (`PROXY_ENV`), `crates/purlis-core/src/sandbox/seatbelt.rs:1073` (`NO_PROXY_ENV`), `crates/purlis-core/src/sandbox/seatbelt.rs:1091` | `network` |
| N34 | Claude Code native sandbox: `allowedDomains` = compiled hosts, `strictAllowlist: true`, `allowUnsandboxedCommands: false`, `failIfUnavailable: true` | `crates/purlis-core/src/sandbox/claude.rs:488`-`:498` | `network` |
| N35 | Claude Code's network goes through purlis's proxy (`httpProxyPort`/`socksProxyPort`) only from Claude Code 2.1.285; older keeps its own proxy and allowed domains, and is not in purlis's network record | `crates/purlis-core/src/sandbox.rs:4645` (`CLAUDE_CODE_TAKES_PROXY_PORTS`), `crates/purlis-core/src/sandbox.rs:4686`, `crates/purlis-core/src/sandbox/claude.rs:633` (`through`), `crates/purlis-core/src/sandbox.rs:7572` | `network` |
| N36 | Claude Code's own web tools (`WebFetch`, `WebSearch`) denied at the full sandbox (they bypass its sandbox); allowed at the floor | `crates/purlis-core/src/sandbox/claude.rs:123` (`WEB_TOOLS`), `crates/purlis-core/src/sandbox/claude.rs:481` | `network` |
| N37 | Codex: live `web_search` disabled; `browser_use`, `computer_use`, `in_app_browser` features disabled; a chat naming `--search`, `--enable`, `--disable`, `-s`, `--yolo` etc. is refused | `crates/purlis-core/src/sandbox/codex.rs:794`, `crates/purlis-core/src/sandbox/codex.rs:803` (`UNMEASURED_FEATURES`), `crates/purlis-core/src/sandbox/codex.rs:813` (`LOOSENING`) | `network` |
| N38 | opencode's `webfetch` runs inside the wrap, so it is held by the proxy like any command (no separate rule) | `crates/purlis-core/src/opencode.rs:131` (tool list), `crates/purlis-core/src/sandbox/opencode.rs:46` | `network` |
| N39 | Certificate check (`com.apple.trustd.agent`, which fetches past the proxy) allowed to sandboxed chats | `crates/purlis-core/src/sandbox.rs:312`, `crates/purlis-core/src/sandbox.rs:367` (`CertificateChecks::of`), Claude `crates/purlis-core/src/sandbox/claude.rs:492` (`enableWeakerNetworkIsolation`), wrap `crates/purlis-core/src/sandbox/seatbelt.rs:353` (`TRUST`) | `network` |
| N40 | The system's name lookup (DNS sockets/services, resolver ports 53/853) is never granted | `crates/purlis-core/src/sandbox/services.rs:398` (`NAME_LOOKUP_SOCKETS`), `crates/purlis-core/src/sandbox/services.rs:413`, `crates/purlis-core/src/sandbox/services.rs:417`, ports `crates/purlis-core/src/sandbox/localport.rs:172` | `network` |
| N41 | Tunnels for clients that skip the proxy (`purlis secret exec` value pointing at host:port): opened only for a host listed **with that port**, exactly one target | `crates/purlis-core/src/sandbox/tunnel.rs:1` (doc), decision via `Reach::decide` | `network` |
| N42 | Tunnel limits: 8 tunnels per run, 16 connections each, 1 h idle | `crates/purlis-core/src/sandbox/tunnel.rs:63`, `:66`, `:70` | `network` |
| N43 | ssh from a chat goes through the chat's SOCKS port; `ControlMaster no`, `ControlPath none` (no riding an outside master connection) | `crates/purlis-core/src/sandbox/tunnel.rs:836` | `network` |
| N44 | A chat planned on another machine (runner): private-network / runner-own addresses reached only if the runner's policy lists them; cloud metadata never | `crates/purlis-core/src/sandbox/internal.rs:37` (`private_address`), `crates/purlis-core/src/sandbox/internal.rs:96`, lock read `crates/purlis-core/src/sandbox/policy.rs:549` | `network` |
| N45 | Docker engine on another machine: plain `tcp://` refused; TLS that does not verify the server refused | `crates/purlis-core/src/sshroute.rs:257` (`PLAIN_TCP`), `crates/purlis-core/src/tlsroute.rs:43` (`NO_VERIFY`) | `network` |
| N46 | Docker TLS files (CA, cert, key) must not be readable by chats: refused if hard-linked or in a folder chats write; key from a vault on disk for one run only | `crates/purlis-core/src/tlsroute.rs:541` (`held_from_chats`), `crates/purlis-core/src/tlsroute.rs:55` (`VAULT_KEY_KEPT_FOR` 600 s) | `network` |
| N47 | SSH route to a remote engine: strict host-key checking against purlis's own pinned known_hosts, no master connection; timeouts | `crates/purlis-core/src/sshroute.rs:546` (`pinned_options`), `crates/purlis-core/src/sshroute.rs:45` (10 s), `:48` (30 s), `:51` (30 s), `:55` (15 s keep-alive) | `network` |
| N48 | Persona's `browser = "project" | "open-web"` declaration marks the browser Ask's answer (grants nothing) | `network` |
| N49 | Settings' Network page / chat Network view list at most 50 newest blocked rows | `app/src-tauri/src/network.rs:30` (`AT_MOST_LISTED`) | `network` |
| N50 | Unknown key in `[sandbox]` or in the policy file is refused (policy file refused whole) | `crates/purlis-core/src/sandbox.rs:834`, `crates/purlis-core/src/sandbox/policy.rs:887` | `network` |

### Unix sockets and local services

| # | Decision | Where | Family |
|---|---|---|---|
| S1 | A sandboxed chat reaches no unix socket but the app's hook socket; `allowAllUnixSockets: false` always written | `crates/purlis-core/src/sandbox/claude.rs:496`, `:541`; wrap floor `crates/purlis-core/src/sandbox/seatbelt.rs:896` | `ports` |
| S2 | The hook socket is added as the first `allowUnixSockets` path and its folder denied to writes | `crates/purlis-core/src/sandbox/claude.rs:647` (`reporting_on`); location `crates/purlis-core/src/hookchannel.rs:70` (`socket_for`: `<plane>/.purlis/app/hooks.sock`, else private temp) | `ports` |
| S3 | The session host's socket folder (`purlisd.sock`) is denied read and write to chats | `crates/purlis-core/src/sandbox.rs:2706` | `ports` |
| S4 | A person may allow a local socket: by whole absolute path, links resolved; refused if hard-linked, owned by another user, in a world-writable folder, not a socket now, under a denied class, or a name-lookup socket | `crates/purlis-core/src/sandbox/services.rs:495` (`socket`) | `ports` |
| S5 | A person may allow a macOS system service (mach-lookup) by whole reverse-DNS name; never a prefix, never keychain services | `crates/purlis-core/src/sandbox/services.rs:656` (`service`), `crates/purlis-core/src/sandbox/services.rs:652` (`NEVER_SERVICES`) | `ports` |
| S6 | Claude Code chats can be allowed no system service by name (Notice offers Start without the sandbox instead) | `app/src-tauri/src/hooks.rs:2104` (`for_its_harness`) | `ports` |
| S7 | Grantable services offered to choose from: none listed | `crates/purlis-core/src/sandbox/services.rs:280` (`GRANTABLE_SERVICES = []`) | `ports` |
| S8 | Socket/service scopes: once (next run), this chat, you (this project on this machine), machine (every project on this machine); **never project** (committed) | `crates/purlis-core/src/sandbox/grant.rs:109` (`machine_wide`), `crates/purlis-core/src/sandbox/grant.rs:116` (`machine_only`), `app/src-tauri/src/sandboxing.rs:1696` | `ports` |
| S9 | Where machine-scope socket/service grants live: `~/.config/purlis/sandbox-grants/…`, never in the project | `crates/purlis-core/src/sandbox/local.rs:1460` (`everywhere_path`), `crates/purlis-core/src/sandbox/local.rs:194` (`path`) | `ports` |
| S10 | A project may **declare** sockets/services it expects (`[sandbox] expects`), which shapes the ask but grants nothing; at most 32 entries, paths ≤ 1024 chars | `crates/purlis-core/src/sandbox/services.rs:722` (`EXPECTS`), `crates/purlis-core/src/sandbox/services.rs:752` (`MOST_DECLARED`), `:747` | `ports` |
| S11 | Container engine / ssh agent / database socket grants carry an honest risk sentence (no extra gate) | `crates/purlis-core/src/sandbox/services.rs:40`-`:50` | `ports` |

## Ports

| # | Decision | Where | Family |
|---|---|---|---|
| P1 | Local binding off for Claude Code: `allowLocalBinding: false` always written (at both levels), so commands connect to no loopback port but the proxy's | `crates/purlis-core/src/sandbox/claude.rs:497`, `:542` | `ports` |
| P2 | Wrapped chats connect to no loopback port (`deny network-outbound remote ip "localhost:*"`) but their proxy's two ports, granted ports and their doors | `crates/purlis-core/src/sandbox/seatbelt.rs:897` (`FLOOR_HELD`), proxy ports `crates/purlis-core/src/sandbox/seatbelt.rs:500`, `:674`; Linux supervisor judges the same profile `crates/linux-sandbox/src/profile.rs:1` | `ports` |
| P3 | Purlis opens one HTTP + one SOCKS5 proxy port per chat on loopback; the chat may reach only its own pair | `crates/purlis-core/src/sandbox/egress.rs:7` (doc), `crates/purlis-core/src/sandbox.rs:5956` (`proxy_ports`), `crates/purlis-core/src/sandbox/localport.rs:216` (`bind_loopback`) | `ports` |
| P4 | A person may allow a "service on this Mac": one loopback port 1..65535, never a range, never purlis's own port, never 53/853 | `crates/purlis-core/src/sandbox/localport.rs:144` (`port`), `crates/purlis-core/src/sandbox/localport.rs:194` (`is_own`), `crates/purlis-core/src/sandbox/localport.rs:172` | `ports` |
| P5 | Port grant scopes: this chat or this project on this machine only (never once, machine or project) | `app/src-tauri/src/sandboxing.rs:1707` | `ports` |
| P6 | A port purlis itself holds is never compiled into a chat; purlis never takes a port a chat was given | `crates/purlis-core/src/sandbox/localport.rs:202` (`still_ports`), `crates/purlis-core/src/sandbox/localport.rs:212` (`BIND_TRIES` 32) | `ports` |
| P7 | Claude Code reaches an allowed local port only by name through its proxy (`this-mac.internal`, `kubernetes.docker.internal`, `host.docker.internal`), never by 127.0.0.1 | `crates/purlis-core/src/sandbox/localport.rs:44` (`NAMES_OF_THIS_MAC`), `crates/purlis-core/src/sandbox/reach.rs:398` (`decide_by_name`) | `ports` |
| P8 | A granted port can be withdrawn live from a running chat | `crates/purlis-core/src/sandbox/reach.rs:421` (`withdraw`), `crates/purlis-core/src/sandbox.rs:6154` | `ports` |
| P9 | A person may allow a chat to **serve** (listen) on one port: ≥ 1024, nothing else listening there, never purlis's own; the server binds a unix socket behind purlis's door on `127.0.0.1:N`/`[::1]:N` | `crates/purlis-core/src/sandbox/localport.rs:390` (`serve_port`), `crates/purlis-core/src/sandbox/localport.rs:385` (`FIRST_SERVED`), `crates/purlis-core/src/sandbox/door.rs:1` | `ports` |
| P10 | Listen grant scopes: this chat or this project on this machine | `app/src-tauri/src/sandboxing.rs:1715` | `ports` |
| P11 | Claude Code chat off macOS cannot be allowed one listen port (its sandbox binds all or none); on macOS its server dials in through `purlis listen`; on a runner it is refused | `app/src-tauri/src/hooks.rs:2120`, `app/src-tauri/src/hooks.rs:2183` (`CLAUDE_CODE_LISTEN_SAID`), `app/src-tauri/src/hooks.rs:1246`; core withdraws listen-only ports for Claude `crates/purlis-core/src/sandbox.rs:7468` | `ports` |
| P12 | A server asking for a port the system picks (`listen(0)`) is refused on macOS, and on Linux on any address but 127.0.0.1 | `app/src-tauri/src/hooks.rs:2194` (`PICKED_PORT_SAID`), `crates/purlis-core/src/sandbox/localport.rs:422` (`PICKED`) | `ports` |
| P13 | On Linux a chat may bind port 0 on 127.0.0.1 with no grant, from a pool of 256 pre-picked ports, and connect only to its own such socket | `crates/linux-sandbox/src/loopback.rs:1`, `crates/linux-sandbox/src/linux/mod.rs:55` (`POOL` 256) | `ports` |
| P14 | Door rules: `Host` must be the door's own (else 421), `Origin` the door's own (else 403), cookies stripped and kept in a door jar, `Transfer-Encoding` refused, one request per connection, carried only to the chat's own socket | `crates/purlis-core/src/sandbox/door.rs:111` (421/403 refusals), `crates/purlis-core/src/sandbox/door.rs:298` (`own_hosts`), `crates/purlis-core/src/sandbox/door.rs:281` | `ports` |
| P15 | Door limits: 30 s head, 32 KiB head, 256 connections, 64 jars, 128 cookies, 4 KiB cookie, 64 KiB jar, 3 s slot wait, 1 s GO answer, 16 slots per door | `crates/purlis-core/src/sandbox/door.rs:57`-`:83` | `ports` |
| P16 | Most parked slots across a chat's doors | `crates/purlis-core/src/sandbox.rs:5476` (`CHAT_SLOTS_MOST` 32) | `ports` |
| P17 | Folders chats' servers listen in are denied to every chat's writes | `crates/purlis-core/src/sandbox.rs:2709` | `ports` |
| P18 | A service on the runner a chat runs on: allowed once or for that chat only, never more chats or this Mac | `app/src-tauri/src/sandboxing.rs:2380` (`RUNNER_SERVICE_SCOPES`), `crates/purlis-core/src/sandbox/localport.rs:76` | `ports` |
| P19 | Ports purlis itself opens and keeps from chats: each chat's proxy pair, each tunnel, each door, forge sign-in loopback callback | `crates/purlis-core/src/sandbox/localport.rs:194` (`is_own`), `crates/purlis-core/src/forge/loopback.rs`, `crates/purlis-core/src/profilestore/sign_in/callback.rs` | `ports` |

## Processes

| # | Decision | Where | Family |
|---|---|---|---|
| 1 | Claude Code's sandbox is always `enabled`, `failIfUnavailable: true` and `allowUnsandboxedCommands: false`, so no command runs outside it at either level | `crates/purlis-core/src/sandbox/claude.rs:489-491` (full), `:536-538` (floor) | `processes` |
| 2 | Where the harness's own hooks and MCP servers run: outside the sandbox for Claude Code on its own sandbox; inside the wrap for Codex, opencode and wrapped Claude | `crates/purlis-core/src/harness/claude.rs:399`, `crates/purlis-core/src/harness/codex.rs:390`, `crates/purlis-core/src/harness/opencode.rs:234` (`sandbox_holds_what_it_starts`) | `processes` |
| 3 | Codex inside the wrap gets `--sandbox danger-full-access` (its own sandbox off), browser_use, computer_use and in_app_browser disabled, `web_search` disabled, and approvals that reject rather than ask | `crates/purlis-core/src/sandbox/codex.rs:774-803` | `processes` |
| 4 | A Codex command line holding a loosening flag (`-s/--sandbox`, bypass/`--yolo`, `--add-dir`, `-C/--cd`, `--approve-for-me`, `--search`, `--enable/--disable`, `-a` other than `never`, or `-c` with a key outside the 12 model/UI keys) is refused | `crates/purlis-core/src/sandbox/codex.rs:813`, `:830`, `:840`, `:946` | `processes` |
| 5 | Wrapped Claude Code: its own sandbox is off inside the wrap, Bash is allowed (since the wrap holds it), and non-essential traffic is off | `crates/purlis-core/src/sandbox/claude_wrap.rs:1-40`, `:175` | `processes` |
| 6 | Wrap base (macOS): `process-exec` and `process-fork` allowed; `signal` and `process-info` only to the same sandbox; a fixed short list of mach services | `crates/purlis-core/src/sandbox/seatbelt.rs:997-1016` (`BASE`) | `processes` |
| 7 | Linux wrap: Landlock (scoped signals, no abstract sockets), a seccomp filter (refuses x32, chown/xattr, non-stream/raw sockets, TCP Fast Open, io_uring, openat2, the key store, open-by-handle, its own seccomp listener) and a supervisor that judges every path and connect call | `crates/linux-sandbox/src/lib.rs:1-25`, `crates/linux-sandbox/src/linux/layers.rs:45`, `crates/linux-sandbox/src/linux/filter.rs:241` | `processes` |
| 8 | Linux wrap: `memfd_create` refused where starts are held to named folders | `crates/linux-sandbox/src/linux/filter.rs:193`, `crates/linux-sandbox/src/profile.rs:329` | `processes` |
| 9 | Linux requires Landlock ABI 6 (Linux 6.12+), else the chat is refused (no backend) | `crates/linux-sandbox/src/lib.rs:45`, `:48`; `crates/purlis-core/src/sandbox/backend.rs:80` | `processes` |
| 10 | On a runner everything a terminal starts runs under `_no-namespaces` (clone/unshare namespaces → EPERM, clone3 → ENOSYS). Only the browser engine may make namespaces | `crates/linux-sandbox/src/lib.rs:71`, `crates/linux-sandbox/src/linux/filter.rs:161`, `crates/purlis-cli/src/main.rs:1333` | `processes` |
| 11 | Floor (wrap, macOS): no Apple Events, no `lsopen`, no job creation; never exec `launchctl`, `osascript`, `open`, `crontab`, `at` or `batch`; no unix socket and no localhost port except what is given back | `crates/purlis-core/src/sandbox/seatbelt.rs:883` (`FLOOR_HELD`) | `processes` |
| 12 | No program starts from any project's vault folder or fingerprint key (for profiles that grant exec by folder) | `crates/purlis-core/src/sandbox/seatbelt.rs:837` | `processes` |
| 13 | The wrap's environment is built by purlis; the chat's own is carried in pieces (`PURLIS_SANDBOX_ENV*`), except `TERM` and `SHELL` | `crates/linux-sandbox/src/carried.rs:22-29` | `processes` |
| 14 | No unix socket except the hook socket and the ones a person allowed; `allowAllUnixSockets: false` | `crates/purlis-core/src/sandbox/claude.rs:496`, `:505`, `:647` (`reporting_on`); `crates/purlis-core/src/sandbox/seatbelt.rs:883` | `processes` |
| 15 | `allowLocalBinding: false` always. An administrator's managed Claude Code setting is read only to report it | `crates/purlis-core/src/sandbox/claude.rs:497`, `:542`, `:847` | `processes` |
| 16 | A container engine's socket (Docker or Podman) reaches a chat only through a socket grant, which says it is "about the same as no sandbox" | `crates/purlis-core/src/sandbox/services.rs:40`, `:148` | `processes` |
| 17 | The profile's program must not be where the chat can write, nor relative. No word of its command may name a writable file. Words longer than 4096 are refused | `crates/purlis-core/src/sandbox/program.rs:60`, `:97`; `crates/purlis-core/src/sandbox.rs:6813-6828` (`ProgramWritable`/`ProgramRelative`/`WordWritable`/`WordTooLong`) | `processes` |
| 18 | Claude Code's program must answer `--version` as `X.Y.Z (Claude Code)` within 10 s, probed with no network and writing only its own temp | `crates/purlis-core/src/sandbox/program.rs:48`, `:498`; `crates/purlis-core/src/sandbox/seatbelt.rs:987` (`probe_profile`) | `processes` |
| 19 | A chat's `PATH` is the app's purlis folder first, then the inherited `PATH`, then a fixed user/system list. Relative and empty entries are dropped | `crates/purlis-core/src/programs.rs:91` (`USER_BIN`), `:113` (`SYSTEM_BIN`), `:374` (`chat_path_from`) | `processes` |
| 20 | A persona's declared tools: a shim folder goes first on `PATH` (and `CLAUDE_ENV_FILE` for Claude) and runs `secret exec`. The real program is resolved where no chat writes | `crates/purlis-core/src/sandbox/tools.rs:1349` (`for_chat`), `:2801`, `:1324` (`resolve`) | `processes` |
| 21 | Names no tool may take (shells, `env`-like runners, purlis's own) and interpreter prefixes | `crates/purlis-core/src/sandbox/tools.rs:369`, `:462` | `processes` |
| 22 | A tool's variable can never be `PATH`, `HOME`, `SHELL`, `IFS`, `ENV`, `BASH_ENV` or `CDPATH`, nor start `LD_*` or `DYLD_*` | `crates/purlis-core/src/sandbox/tools.rs:725`, `:729` | `processes` |
| 23 | Run grants: never offered for programs that run others; "any arguments" only for `curl` | `crates/purlis-core/src/sandbox/tools.rs:1736`, `:2222`; `crates/purlis-core/src/sandbox/policy.rs:765` | `processes` |
| 24 | A chat starts from an empty environment plus a keep-list (PATH, HOME, locale, proxies, XDG, SSH_AUTH_SOCK, CA files, `PURLIS_*`/`CHARTER_*`, Windows basics) | `crates/purlis-core/src/chatenv.rs:55` (`PASSED`), `:396` (`compose`) | `processes` |
| 25 | Plus the harness's own namespace: `CLAUDE_*`, `CODEX_*`, `OPENCODE_*` | `crates/purlis-core/src/harness_declaration/claude.toml:10`, `codex.toml:10`, `opencode.toml:10`; `crates/purlis-core/src/harness.rs:233` | `processes` |
| 26 | Credential-like variables are not passed unless the person lists the exact name. That covers names holding KEY, TOKEN, SECRET or PASSWORD, and the families GITHUB_\*, GH_\*, AWS_\*, GOOGLE_\*, ANTHROPIC_\*, OPENAI_\*, NPM_\*, DOCKER_\*, VAULT_\* and KUBECONFIG. A prefix entry never admits one | `crates/purlis-core/src/chatenv.rs:138` (`CREDENTIALS`), `:198` (`passes`); `crates/purlis-core/src/profiles.rs:52` (`SECRET_WORDS`) | `processes` |
| 27 | `[chat_env] pass` is read only from this machine's local file; a git-carried one passes nothing | `crates/purlis-core/src/chatenv.rs:582` | `processes` |
| 28 | Never passed, whoever lists it: `OP_*` (any case), every vault identity variable, `TERM`, and `NOT_INHERITED` (harness identity, chat token, workspace/root pins, sandboxed/wrapped markers, `CLAUDE_CODE_SANDBOXED`, …) | `crates/purlis-core/src/chatenv.rs:237` (`inherited`); `crates/purlis-core/src/hookwire.rs:122`, `:142`; `crates/purlis-core/src/secrets/identity.rs:1340` | `processes` |
| 29 | A project-declared harness's `env` namespace is judged one variable at a time (credential words, plus CREDENTIAL, AUTH, PASSWD, PASSPHRASE, COOKIE and PRIVATE refused) | `crates/purlis-core/src/chatenv.rs:293`, `:319` | `processes` |
| 30 | A profile's `env` may not set credential-named variables or the harness-internal variable | `crates/purlis-core/src/profiles.rs:52`, `crates/purlis-core/src/hookwire.rs:145` | `processes` |
| 31 | A wrapped chat: proxy variables point at purlis's proxy, `NO_PROXY` is emptied, and `TMPDIR`/`TMP`/`TEMP`/`xcrun_db`/`CLANG_MODULE_CACHE_PATH` point at its own fresh temp | `crates/purlis-core/src/sandbox/seatbelt.rs:1062`, `:1073`, `:1080`, `:1091` | `processes` |
| 32 | Harness homes by env: `CODEX_HOME` (profile folder), opencode `XDG_STATE_HOME` (chat's own), wrapped Claude `CLAUDE_CODE_TMPDIR` (≤44 chars) | `crates/purlis-core/src/sandbox/codex.rs:135`, `crates/purlis-core/src/sandbox/opencode.rs:225`, `crates/purlis-core/src/sandbox/claude_wrap.rs:129`, `:136` | `processes` |
| 33 | Git hooks (pre-commit, pre-merge-commit, pre-push) are armed in every chat through `GIT_CONFIG_COUNT`/`core.hooksPath` | `crates/purlis-core/src/githooks.rs:51-68`, `:144` (`arm`) | `processes` |
| 34 | macOS: a chat's commands never read or write another terminal (`/dev/ttys*`) except its own | `crates/purlis-core/src/sandbox/claude.rs:164`, `:228`; `crates/purlis-core/src/sandbox/seatbelt.rs:1030` | `processes` |
| 35 | `own-terminal-only`: also no terminal the chat makes itself (`script`, `tmux`, `expect` fail) | `crates/purlis-core/src/sandbox.rs:346`, `:904`; `crates/purlis-core/src/sandbox/seatbelt.rs:1050` | `processes` |
| 36 | The chat host leaves its controlling terminal. No chat runs over ACP while purlis still holds the terminal it was started from | `crates/purlis-core/src/noterminal.rs:1-10`, `crates/purlis-core/src/acp.rs:296-298` | `processes` |
| 37 | An ACP agent runs in its own process group, which is killed whole; `initialize` offers no `fs` and no `terminal` | `crates/purlis-core/src/acp.rs:950`, `:962`, `:15` | `processes` |
| 38 | Kill switch: when it is set, no chat starts (the operator's own shell excepted) | `crates/purlis-core/src/halt.rs:32`; `app/src-tauri/src/hostedsessions.rs:4744`, `:4898`, `:4948`; `app/src-tauri/src/killswitch.rs:147` | `fixed` |
| 39 | Hook/host sockets admit only a peer of the same uid (unmapped uid refused). The daemon refuses human scopes to any process inside a chat | `crates/same-user/src/lib.rs:186` (`admit_peer`), `crates/same-user/src/ancestry.rs:276` (`inside_a_chat`), `crates/session-protocol/src/local.rs:91` | `processes` |
| 40 | Runner containers: `--cap-drop ALL`, `no-new-privileges`, read-only root, non-root user, image by digest. Defaults: 2 CPUs, 4096 MiB, 1024 processes (at least 64), 1024 MiB `/tmp` | `crates/purlis-core/src/dockerpool.rs:465-505`, `:734-750`; `crates/purlis-core/src/poollimits.rs:361-418` | `processes` |
| 41 | A chat's kubeconfig: only a copy for one local-cluster context (127.0.0.1, localhost or kubernetes.docker.internal over HTTPS), made on the person's press. A context with its own proxy or with TLS checks skipped is refused | `crates/purlis-core/src/sandbox/kubeconfig.rs:34`, `:61`, `:94`; `app/src-tauri/src/kubectl.rs:62` | `processes` |
| 42 | `DOCKER_*` and `KUBECONFIG` never pass from the app's environment by default (see P26). purlis's own `docker` runs clear `DOCKER_CONTEXT` | `crates/purlis-core/src/chatenv.rs:138`; `crates/purlis-core/src/dockercontext.rs:1-10` | `processes` |
| 43 | Extensions run with no OS sandbox, as the person (ruling 2026-09-22). Changes outside their declared writes are only watched and reported | `crates/purlis-core/src/executor.rs:56-63`, `crates/purlis-core/src/executor/watch.rs` | `processes` |
| 44 | An extension that declares a studio capability runs confined: it reads its own folder and system program folders only, writes its state and temp only, runs programs only from read-only places, and reaches only its proxy | `crates/purlis-core/src/sandbox/confined.rs:228`, `:296`, `:364` | `processes` |
| 45 | One question per process: 5 s deadline, 250 ms grace, 512 KiB answer, 2 KiB stderr kept, 8 MiB check output, process group killed after | `crates/purlis-core/src/executor.rs:166-199`, `:3005` | `processes` |
| 46 | Every descriptor above 2 is marked close-on-exec in `pre_exec` (audited `unsafe` #1) | `crates/purlis-core/src/executor.rs:1988`, `:1940` | `processes` |
| 47 | The extension's environment: only PATH, HOME, USER, LOGNAME, LANG, LC_ALL, LC_CTYPE and TMPDIR, plus `PURLIS_EXTENSION*` | `crates/purlis-core/src/executor.rs:2590` | `processes` |
| 48 | The extension is re-approved and re-fingerprinted at each press; its program must be an executable file run directly (no shell); not on Windows | `crates/purlis-core/src/executor.rs:2257`, `:2566`, `:207`, `:214` | `processes` |

## Guards


### Commit gate (userpromptsubmit nudge)

| # | Decision | Where | Family |
|---|---|---|---|
| 1 | A work prompt that leaves a fork open (open-ended wording, broad scope, something irreversible, a long many-part ask) gets a "scout, then ask at the fork" message before work | crates/purlis-core/src/commitgate.rs:137 (`signals`), :178 (`message`) | `guards.save` |
| 2 | Gate fires only in a plane and only for an attended chat (attendance fact, never payload `permission_mode`) | crates/purlis-core/src/commitgate.rs:215 | `guards.save` |
| 3 | Gate fires only on a session's first prompt, or a later one that opens with an ask for a code change (>= 5 words, not a slash command or question) | crates/purlis-core/src/commitgate.rs:168, :65 (`SHORT_REPLY_WORDS = 5`) | `guards.save` |
| 4 | After firing, gate stays quiet for N prompts | crates/purlis-core/src/commitgate.rs:62 (`COOLDOWN = 3`) | `guards.save` |
| 5 | Prose length that makes an ask "many-part" | crates/purlis-core/src/commitgate.rs:68 (`PROSE_LONG = 240`) | `guards.save` |

### Commit guard (Bash guard arm A8, `git-hook-skip`)

| # | Decision | Where | Family |
|---|---|---|---|
| 6 | Refuse `git commit --no-verify` / `-n` (also in a bundle like `-an`) | crates/purlis-core/src/commitguard.rs:127 (`commit_skips_hooks`), arm at crates/purlis-core/src/toolgate.rs:627 | `guards.save` |
| 7 | Refuse `git merge --no-verify` and `git push --no-verify` (and prefixes from `--no-veri`) | crates/purlis-core/src/commitguard.rs:160, :244 | `guards.save` |
| 8 | Refuse `git send-pack` (push without `pre-push`) | crates/purlis-core/src/commitguard.rs:249, :255 | `guards.save` |
| 9 | Refuse `git -c core.hooksPath=…` / `--config-env=core.hooksPath=…` on any subcommand | crates/purlis-core/src/commitguard.rs:116, :229 | `guards.save` |
| 10 | Refuse `GIT_CONFIG_COUNT` / `GIT_CONFIG_PARAMETERS` / `GIT_CONFIG_KEY_n`/`VALUE_n` set or exported on the line (would replace the chat's hook pair) | crates/purlis-core/src/commitguard.rs:107, :197 | `guards.save` |
| 11 | Refuse `git revert` of a commit that changed `.charter-scan-allow.toml`, and `git merge --ff-only` onto a commit whose allowlist differs from HEAD | crates/purlis-core/src/commitguard.rs:292 (`moves_the_allowlist`), :268 | `guards.save` |
| 12 | Time the guard waits for git to answer #11; a git that does not answer lets the command through (fails open) | crates/purlis-core/src/commitguard.rs:286 (`ASKING_GIT = 3s`) | `guards.save` |
| 13 | Refuse a chat running `purlis doctor --fix rename-plane` (fails closed on unreadable spellings) | crates/purlis-core/src/commitguard.rs:61, :169 | `guards.plane` |

### Git hooks a chat is armed with (pre-commit / pre-merge-commit / pre-push / commit-msg)

| # | Decision | Where | Family |
|---|---|---|---|
| 14 | Every chat's git gets `core.hooksPath` = app-owned hooks dir via `GIT_CONFIG_COUNT` pair appended after any existing pairs | crates/purlis-core/src/githooks.rs:144 (`arm`), applied at crates/purlis-core/src/chatenv.rs:424 | `guards.git` |
| 15 | Which repo hooks get a shim (the repo's own hook still runs after purlis's check); `post-index-change`, `reference-transaction`, `fsmonitor-watchman` of a repo do NOT run in a chat | crates/purlis-core/src/githooks.rs:86 (`NAMES`, 25 hooks) | `guards.git` |
| 16 | `pre-commit` / `pre-merge-commit`: staged diff scanned for secrets/personal data; a finding refuses the commit and raises a needs-you item; unscannable = refuse | crates/purlis-cli/src/githook.rs:28 (`run`), crates/purlis-core/src/githooks.rs:59 (`CHECKED`) | `guards.credential` |
| 17 | `pre-commit` (not merge) refuses a commit that changes the scan's allowlist | crates/purlis-cli/src/githook.rs:52 | `guards.save` |
| 18 | `pre-commit` refuses a staged `charter.toml` whose `[sandbox]`, `[chat_env]` or `[dispatch]` differ from the working-tree file (#1464) | crates/purlis-cli/src/githook.rs:62 | `fixed` |
| 19 | `pre-push`: every commit the remote lacks is scanned; a finding or an allowlist change refuses the push; unreadable input refuses | crates/purlis-cli/src/githook.rs:121 (`push`) | `guards.credential` |
| 20 | `commit-msg`: a message holding a credential shape is refused (exit 3); unreadable message refused; otherwise provenance trailers stamped | crates/purlis-cli/src/githook.rs:91 (`message`), crates/purlis-core/src/githooks.rs:79 (`MESSAGE_REFUSED = 3`) | `guards.credential` |

### Branch-folder git (Bash guard arm A9b, `branch-folder-git`) and project git (A9, `project-git-lock`)

| # | Decision | Where | Family |
|---|---|---|---|
| 21 | Sandboxed chat: `git add` / `git commit` (non-dry-run) in a branch folder purlis made is refused with "use `purlis worktree commit`" | crates/purlis-core/src/branchgit.rs:142 (`refusal`), :1133 (`writes_the_index`); arm crates/purlis-core/src/toolgate.rs:657 | `sandbox` |
| 22 | Sandboxed chat outside the project root: git subcommands that take a lock in the project's own `.git` are refused (fails open where it cannot place the command) | crates/purlis-core/src/projectgit.rs:103 (`refusal`), :43 (`LOCKING`, 18 verbs); arm crates/purlis-core/src/toolgate.rs:642 | `sandbox` |
| 23 | Most `cd` places followed on one line before the shell's folder is "unknown" | crates/purlis-core/src/branchgit.rs:61 (`MOST_PLACES = 8`) | `sandbox` |
| 24 | Unsandboxed chat: any git that may write a branch folder's branch/HEAD/index/tree but cannot be placed (unfollowable cd, alias, wrapper, script, nested shell) is refused with the shape that places it, where the project has any branch folder | crates/purlis-cli/src/guard.rs:278; crates/purlis-core/src/branchgit.rs:183 (`branch_write`), :350 (`ONLY_READ` allow-list: unknown subcommand counts as a write) | `guards.git` |
| 25 | Nesting depth / start-file read size the unsandboxed reader follows | crates/purlis-core/src/branchgit.rs:872 (`MOST_NESTED = 4`), :240 (`START_FILE_READ = 1 MiB`) | `guards.git` |

### One live writer per branch folder (piece claim)

| # | Decision | Where | Family |
|---|---|---|---|
| 26 | A second chat's commit/update/push/remove in a branch folder another live chat holds is refused; first writer takes the claim | crates/purlis-core/src/piececlaim.rs:320 (`take`), :415 (`removing`) | `guards.git` |
| 27 | Holder liveness unreadable (chat not back since restart, no app) → brokered write refused (fail closed) | crates/purlis-core/src/piececlaim.rs:22 (doc), :352 | `guards.git` |
| 28 | Plain-git claim check: no app answer within budget lets the git run (fail open) | crates/purlis-cli/src/guard.rs:253 (`CLAIMS_ARE_ANSWERED_WITHIN = 900ms`); crates/purlis-cli/src/gitask.rs:337 (`A_CLAIM_TAKES_AT_MOST = 2s`) | `guards.git` |
| 29 | Claim lock wait | crates/purlis-core/src/piececlaim.rs:281 (`A_CLAIM_WAITS_AT_MOST = 5s`) | `guards.git` |

### Git broker (clone, worktree add/cut/update/push/remove/prune, root update/save, clone update)

| # | Decision | Where | Family |
|---|---|---|---|
| 30 | `purlis clone` / `worktree add | update | `guards.git` |
| 31 | Chat on a pool's runner: app does no clone / branch folder / commit on the person's machine for it | app/src-tauri/src/gitbroker.rs:147 (`on_a_runner`) | `runners` |
| 32 | Workspace bound: chat may ask only for the workspace it stands in, or any from the project root | crates/purlis-core/src/gitbroker.rs:679 (`bound`) | `guards.git` |
| 33 | `workspace create` via broker: chat must stand in the project | crates/purlis-core/src/gitbroker.rs:905 (`in_the_project`) | `guards.git` |
| 34 | Branch-folder update/push: chat stands in that folder or above it, never in another branch folder | crates/purlis-core/src/gitbroker.rs:707 (`at_or_above`) | `guards.git` |
| 35 | Branch-folder remove: only from above it, never from inside; never forced; refused where uncommitted work / unreachable commits / ignored files would be lost | crates/purlis-core/src/gitbroker.rs:727 (`above`), :326-:335 | `guards.git` |
| 36 | Project-root update only for the chat standing at the root | crates/purlis-core/src/gitbroker.rs:751 (`at_the_root`) | `guards.plane` |
| 37 | Project save for a chat: chat stands in the project; app writes the root's git only where the chat's sandbox holds it (else "left to the chat") | crates/purlis-core/src/gitbroker.rs:765 (`saved_from`), :207 | `guards.plane` |
| 38 | `purlis save --pull` from a chat refused | crates/purlis-core/src/gitbroker.rs:457, :816 (`SAVE_PULL_FOR_A_CHAT`) | `guards.git` |
| 39 | Clone update: chat stands in that clone, the workspace folder, or project root | crates/purlis-core/src/gitbroker.rs:848 (`at_or_above_clone`) | `guards.git` |
| 40 | Hosts a brokered clone/fetch/push may reach = sandbox egress where the chat runs sandboxed; none (no limit) for unsandboxed; unreadable charter.toml refuses | crates/purlis-core/src/gitbroker.rs:928 (`hosts`), :938 (`sandboxed_hosts`) | `network` |
| 41 | Repo whose own config names a program (filter, include/includeIf, diff/merge driver, sshCommand, gitProxy, uploadpack/receivepack/vcs, gc hook, credential helper except purlis's own, trailer cmd, hook.*) is refused before any git runs | crates/purlis-core/src/gitbroker.rs:1246 (`names_a_program`), :1180 (`checked_repo`), :1287 (policy helper exemption) | `guards.git` |
| 42 | Every brokered git reads no global/system config; hooks and fsmonitor off; bare repos only when named; no auto-gc | crates/purlis-core/src/worktree/git.rs:321 (`NO_PROGRAMS`), :357, :772 | `guards.git` |
| 43 | Repo `.git` must be the clone's own real dir (no link, no other git dir); pinned by inode | crates/purlis-core/src/gitbroker.rs:1180 | `guards.git` |
| 44 | Answer size cap to the chat | crates/purlis-core/src/gitbroker.rs:963 (`SAID_AT_MOST = 56 KiB`), :966 (`A_LINE_AT_MOST = 2000`) | `limits` |
| 45 | App-written trailers (`purlis-chat`, `purlis-persona`, `purlis-change`, `co-authored-by`, `assisted-by`) stripped from a chat's save message | crates/purlis-core/src/gitbroker.rs:788 (`TRAILERS_THE_APP_WRITES`) | `guards.git` |
| 46 | CLI wait budgets for brokered work (clone silent 5 min + 120 s; worktree add 120 s; piece update/push 3x120 s + 8x30 s; save; commit; prune per-folder 240 s) | crates/purlis-cli/src/gitask.rs:39, :44, :78, :126, :132, :142 | `limits` |

### Brokered commit (`purlis worktree commit`)

| # | Decision | Where | Family |
|---|---|---|---|
| 47 | Sandboxed chat's commit goes through the app; unsandboxed chat/terminal commits by itself with its own git, hooks, signer | crates/purlis-cli/src/gitask.rs:306; crates/purlis-core/src/gitbroker/commit/plain.rs:4 | `guards.git` |
| 48 | Unsandboxed chat's `worktree commit` refused, pointing at plain git | crates/purlis-core/src/gitbroker/commit.rs:101 (`refuses_unsandboxed`), crates/purlis-core/src/planesave.rs:164 | `guards.git` |
| 49 | Brokered commit never amends, resets, rebases, merges, pushes (`--amend` refused) | crates/purlis-cli/src/gitask.rs:154; crates/purlis-core/src/gitbroker/commit.rs:88 (`COMMITS_ONLY`) | `guards.git` |
| 50 | Commit message limits: non-empty, <= 32 KiB, no NUL | crates/purlis-core/src/gitbroker/commit.rs:721 (`message_ok`), :79 | `guards.git` |
| 51 | Paths: 1..1000, each <= 4096 bytes, relative, no `..`, no `.git`, no control chars, none through a link | crates/purlis-core/src/gitbroker/commit.rs:741, :82, :85, :786 | `guards.git` |
| 52 | A message carrying purlis's own provenance trailers is refused | crates/purlis-core/src/gitbroker/commit.rs:455 (`trailers_ok`) | `guards.git` |
| 53 | Rate cap per chat per project | crates/purlis-core/src/gitbroker/commit.rs:416 (`COMMITS_A_WHILE = 30`), :419 (`COMMITS_WITHIN = 10 min`) | `limits` |
| 54 | Staging / commit deadlines | crates/purlis-core/src/gitbroker/commit.rs:110 (`STAGING = 5 min`), :113 (`COMMITTING = 2 min`) | `limits` |
| 55 | Repo's own `pre-commit`, `prepare-commit-msg`, `commit-msg`, `post-commit` do not run (purlis's scan does) | crates/purlis-core/src/gitbroker/commit.rs:137 (`HOOKS`), :123 (`COMMIT` argv: `--no-gpg-sign`, `--no-edit`, gc off) | `guards.git` |
| 56 | Repo with `config.worktree` or `objects/info/alternates` refused | crates/purlis-core/src/gitbroker/commit.rs:873 (`borrows_nothing`) | `guards.git` |
| 57 | Repo with `commit.gpgsign = true` or naming a signer (`gpg.program`, `gpg.*.program`, `gpg.ssh.defaultKeyCommand`) refused | crates/purlis-core/src/gitbroker/commit.rs:502, :980; crates/purlis-core/src/gitbroker.rs:1274 | `guards.git` |
| 58 | Staged result refused (and unstaged) if it records a gitlink, changes `.gitmodules`, adds a case-twin name, or holds a content-filtered (e.g. LFS) file; a hard-linked file refused | crates/purlis-core/src/gitbroker/commit.rs:1229, :1257, :1293, :1093 | `guards.git` |
| 59 | Detached HEAD: no commit | crates/purlis-core/src/gitbroker/commit.rs:522 | `guards.git` |
| 60 | Commit made as the email chosen for the forge account of `origin`'s host | crates/purlis-core/src/gitbroker/commit.rs:493; app/src-tauri/src/gitbroker.rs:44 | `guards.git` |

### Brokered update / push (`purlis worktree update|push`)

| # | Decision | Where | Family |
|---|---|---|---|
| 61 | Push to the remote's default branch refused: a branch folder pushes its own branch only | crates/purlis-core/src/pieceupdate.rs:1854 | `guards.git` |
| 62 | Force push only as `--force-with-lease` to the exact commit purlis's own update recorded; any other rewrite / remote moved refused | crates/purlis-core/src/pieceupdate.rs:1872-:1876, :1899, :2062 (`vouched`) | `guards.git` |
| 63 | Push with an operation stopped (rebase/merge mid-way) refused | crates/purlis-core/src/pieceupdate.rs:1841 | `guards.git` |
| 64 | Update refused with uncommitted changes, or where it would overwrite an untracked/ignored file; for a sandboxed chat, a rebase that would write a file the sandbox denies is refused | crates/purlis-core/src/pieceupdate.rs:733, :932 | `guards.git` |
| 65 | A chat's push is never signed with the person's key (`--no-signed`) | crates/purlis-core/src/pieceupdate.rs:1905 | `guards.git` |
| 66 | Push refused by forge for unsigned commits is kept as waiting work; window offers signing setup; chat never runs setup | crates/purlis-core/src/gitbroker.rs:45 (`waits_on_signing`), crates/purlis-core/src/pieceupdate.rs:1924 | `guards.git` |

### Signing and clone git policy

| # | Decision | Where | Family |
|---|---|---|---|
| 67 | Commits the app makes for a chat are signed with this device's Ed25519 key, if set up; else made unsigned with a note | app/src-tauri/src/gitbroker.rs:44 (`signing_isolation`); crates/purlis-core/src/gitbroker/commit.rs:672 (`signed`); key at crates/purlis-core/src/commitsign.rs:37 (`SERVICE`, Keyring tier) | `guards.git` |
| 68 | Chats never reach the signing key (held to the app; not in env/files/answers) | crates/purlis-core/src/commitsign.rs:17-:21 (doc), :140 | `guards.credential` |
| 69 | A save the app makes for a chat is never signed, even with `--sign` or `[plane] sign` | crates/purlis-core/src/planegit.rs:2160 (`SIGN_LEFT_FOR_A_CHAT`), :3631 | `guards.git` |
| 70 | Every clone purlis makes gets local config: forge CLI credential helper, `commit.gpgsign=false`, `tag.gpgsign=false`, SSH->HTTPS `insteadOf` | crates/purlis-core/src/gitpolicy.rs:62 (`apply`), :127 (`check`) | `guards.git` |

### Git config read for plane-root guards

| # | Decision | Where | Family |
|---|---|---|---|
| 71 | `core.worktree` from a repo's `.git/config` is read as a guard subject (fails to "none named"); only a regular file, bounded | crates/purlis-core/src/gitconfig.rs:78, :61 (`MAX_CONFIG_BYTES = 256 KiB`), :64 (`MAX_POINTER_BYTES = 4096`) | `guards.plane` |

### Leak guard (arm A: a command that would print a secret into the transcript)

| # | Decision | Where | Family |
|---|---|---|---|
| L1 | Leak arm runs first and **ungated**: outside a plane too, before every other arm; a hit is a hard deny | `crates/purlis-core/src/toolgate.rs:429` | `secrets` |
| L2 | A reader whose file operand names a vault path is denied (`READ_REASON`) | `crates/purlis-core/src/leakguard.rs:1796` (`opens` at :1845) | `secrets` |
| L3 | Which programs count as "readers": 16 fixed names (cat, less, more, head, tail, bat, nl, tac, xxd, od, strings, grep, rg, ag, awk, sed); deliberately not widened | `crates/purlis-core/src/leakguard.rs:105` | `secrets` |
| L4 | What a "vault path" is: `.purlis`/`.charter`/`.edm` dir itself, `/vaults` segment, `/browser`, `/active-*`, `/fingerprint*`; case-insensitive | `crates/purlis-core/src/leakguard.rs:401` (`vault_path_re`) | `secrets` |
| L5 | `purlis`/`charter` invoked with `--reveal` is denied (`REVEAL_REASON`) | `crates/purlis-core/src/leakguard.rs:1772`; recognised names `CHARTER_PROGS` :120 | `secrets` |
| L6 | `script … purlis … --reveal` (pty relay) is denied | `crates/purlis-core/src/leakguard.rs:1783` | `secrets` |
| L7 | Input redirection / `xargs -a` of a vault path denied before the program is known | `crates/purlis-core/src/leakguard.rs:1766` | `secrets` |
| L8 | `gh`/`glab` upload of a vault file (body/notes/template file, `api --input`, `-F @path`, release-asset operands, …) denied; upload table | `crates/purlis-core/src/leakguard.rs:1810`; table `FORGE_UPLOADS` :784 | `secrets` |
| L9 | A tree walk (grep -r/-R, rg, ag) that would descend into guarded state (`vaults`, `browser*`, `active-*`, `fingerprint*`) is denied even when no path is named | `crates/purlis-core/src/leakguard.rs:1824`; walkers `TREE_WALKERS` :142; guarded names `GUARDED_STATE_EXACT` :128, `GUARDED_STATE_PREFIXES` :133 | `secrets` |
| L10 | Exclusion options that make a walk allowed (`--exclude-dir`, `--exclude`, `--ignore-dir`, rg `-g '!…'`) | `crates/purlis-core/src/leakguard.rs:163`, `:168` | `secrets` |
| L11 | Directory moves followed (cd, pushd, `env -C`, `sudo --chdir`); `popd` not followed | `crates/purlis-core/src/leakguard.rs:115` | `secrets` |
| L12 | An unparseable command line is scanned as raw text for `--reveal` or a vault path (fail closed) | `crates/purlis-core/src/leakguard.rs:1691` (`as_text`) | `secrets` |
| L13 | Command substitutions read from at most 8 directories; past that the call is refused as too big to check | `crates/purlis-core/src/leakguard.rs:1533` (`RELOCATIONS`), applied :1653 | `secrets` |
| L14 | Substitution nesting cap (8) and read budget (4x command length, at least 64 KiB); past either -> refuse "too big to check" | `crates/purlis-core/src/guardcaps.rs:54` (`MAX_NESTING`); `crates/purlis-core/src/shellsubst.rs:37`, `:40` | `secrets` |

### Read/Grep vault guard (twin of the leak guard on file tools)

| # | Decision | Where | Family |
|---|---|---|---|
| R1 | `Read`/`Grep` naming a vault path is denied; **not plane-gated** | `crates/purlis-core/src/toolhooks.rs:333` | `secrets` |
| R2 | `Grep` (no path = cwd) that walks into guarded state is denied | `crates/purlis-core/src/toolhooks.rs:351` | `secrets` |
| R3 | A `Grep` `glob` that provably selects nothing inside the guarded entry is allowed; look-ahead bounded at 512 entries and fails closed past it | `crates/purlis-core/src/toolhooks.rs:362` (limit 512); `crates/purlis-core/src/leakguard.rs:1330` | `secrets` |
| R4 | A judge chat (env `JUDGE_FOLDER_ENV` set) skips the vault guard (its own folder rule applies instead) | `crates/purlis-core/src/toolhooks.rs:328` | `secrets` |

### Credential guard (arm A2: one credential per forge, token over HTTPS)

| # | Decision | Where | Family |
|---|---|---|---|
| C1 | Arm A2 runs only inside a plane (after the leak arm) | `crates/purlis-core/src/toolgate.rs:436` | `guards.credential` |
| C2 | `git` with `GIT_SSH`/`GIT_SSH_COMMAND` in its env (attached or exported earlier) denied | `crates/purlis-core/src/credguard.rs:397` | `guards.credential` |
| C3 | `GIT_CONFIG_KEY_n=core.sshCommand` env spelling denied | `crates/purlis-core/src/credguard.rs:408` | `guards.credential` |
| C4 | `git -c core.sshCommand=…` / `--config-env=core.sshCommand` denied | `crates/purlis-core/src/credguard.rs:417`, `:425` | `guards.credential` |
| C5 | `git config core.sshCommand <v>` (a write) denied; reading it allowed | `crates/purlis-core/src/credguard.rs:435` | `guards.credential` |
| C6 | `git` with an SSH URL to a known forge as an operand denied (use HTTPS) | `crates/purlis-core/src/credguard.rs:448` | `guards.credential` |
| C7 | Commit/tag signing denied: `--gpg-sign`, `-c commit/tag.gpgsign=true`, `-S` on commit/tag/merge/revert/cherry-pick/rebase/am, `-s/--sign` on tag | `crates/purlis-core/src/credguard.rs:474-498`; verbs `SIGN_VERBS` :150 | `guards.credential` |
| C8 | `ssh git@<known forge>` denied (check with `<cli> auth status`) | `crates/purlis-core/src/credguard.rs:501` | `guards.credential` |

### Commit / push diff scan (SQ-16, SQ-7)

| # | Decision | Where | Family |
|---|---|---|---|
| D1 | A chat's `pre-commit` / `pre-merge-commit` refuses a commit whose added lines hold a credential or personal data (masked findings, needs-you item raised) | `crates/purlis-cli/src/githook.rs:43-75`; `crates/purlis-core/src/diffscan.rs:97` (`checked`); hooks checked `crates/purlis-core/src/githooks.rs:59` | `guards.credential` |
| D2 | A chat's `commit-msg` refuses a message holding a **credential** (personal data allowed in messages) | `crates/purlis-cli/src/githook.rs:85`; `crates/purlis-core/src/diffscan.rs:135`, `:436` (`in_message`) | `guards.credential` |
| D3 | Below git's scissors line in a message, a credential already held at HEAD/index in a changed file is let through; anything else read | `crates/purlis-core/src/diffscan.rs:135-145`, `:200` | `guards.credential` |
| D4 | A chat's `pre-push` scans every commit (diffs incl. merge resolutions, commit and tag messages, LFS objects) the remote lacks; refuses on a finding, on unreadable input, or on a ref that is not a commit | `crates/purlis-cli/src/githook.rs:108-140`; `crates/purlis-core/src/diffscan.rs:729` (`pushed`), `:405` (LFS) | `guards.credential` |
| D5 | Remote that cannot be asked what it has: push scan reads at most 1000 commits, else refuses outright | `crates/purlis-core/src/diffscan.rs:686` (`UNASKED_AT_MOST`), `:1020` | `guards.credential` |
| D6 | At most 4096 remote commits are taken as "already known" (past that, scan more, never less) | `crates/purlis-core/src/diffscan.rs:567` (`KNOWN_AT_MOST`) | `guards.credential` |
| D7 | Files git shows as binary are read as text up to 10 MiB (plane-save limit); bigger ones not read; `core.bigFileThreshold` forced to 512m | `crates/purlis-core/src/diffscan.rs:1119` (`PROBE_MOST` = `planegit.rs:2811 SCAN_LIMIT`), `:1111`, `:1115` | `guards.credential` |
| D8 | A chat's commit that changes the scan allowlist file is refused (merges exempt) | `crates/purlis-cli/src/githook.rs:52`; `crates/purlis-core/src/diffscan.rs:1805` | `guards.save` |
| D9 | A chat's push of a range that changes the allowlist is refused | `crates/purlis-cli/src/githook.rs:126`; `crates/purlis-core/src/diffscan.rs:1820` | `guards.save` |
| D10 | A chat's commit staging a project file (`purlis.toml`/local settings) whose `[sandbox]`, `[chat_env]` or `[dispatch]` differs from the working tree (or is unreadable / not a regular file) is refused | `crates/purlis-cli/src/githook.rs:62`; `crates/purlis-core/src/diffscan.rs:264`, `:1812` | `fixed` |
| D11 | A refusal lists at most 20 findings; values always masked (first <=4 chars, at most 32 shown) | `crates/purlis-core/src/diffscan.rs:1826` (`LISTED`); `crates/purlis-core/src/secretshape.rs:2903` (`MASK_WIDTH`), `:2907` | `guards.credential` |
| D12 | `purlis scan [--explain]`: dry run of the pre-commit scan on what is staged; exit 1 on a finding | `crates/purlis-cli/src/scan.rs:20` | `guards.credential` |

### Scan allowlist (`.purlis-scan-allow.toml`, legacy `.charter-scan-allow.toml`)

| # | Decision | Where | Family |
|---|---|---|---|
| A1 | Allowlist file name | `crates/purlis-core/src/names.rs:290` (`SCAN_ALLOW`); `crates/purlis-core/src/scanallow.rs:36` | `guards.save` |
| A2 | Allowlist is read as committed at `HEAD` (push: as the remote has it), never working tree/index | `crates/purlis-core/src/diffscan.rs:319`, `:326` | `guards.save` |
| A3 | Built-in entry: `email` rule allowed in Cargo.toml, package.json, .mailmap, AUTHORS*, CONTRIBUTORS*, CHANGELOG(.md) at any depth | `crates/purlis-core/src/scanallow.rs:127`, `:144` | `guards.save` |
| A4 | Entry shape: `rule` + `paths` globs, or `fingerprint` (sha256 of value), always with `reason`; rule needs paths (`["**"]` for whole repo); invalid entries allow nothing | `crates/purlis-core/src/scanallow.rs:241` (`entry`), `:224` (`allowing`) | `guards.save` |
| A5 | Personal-data rules can never be allowed by fingerprint, only by path | `crates/purlis-core/src/scanallow.rs:232`, `:278` | `guards.save` |

### Secret shapes (the shared classifier)

| # | Decision | Where | Family |
|---|---|---|---|
| S1 | Core rules: AgentMail key, JWT, PEM private key, AWS key, credential assignment (`token: <6+ chars>`) | `crates/purlis-core/src/secretshape.rs:200` (`checks`) | `secrets` |
| S2 | 36 forge/vendor token prefixes (`ghp_`, `glpat-`, `xoxb-`, …), >= 16 body chars (npm: alphanumeric) | `crates/purlis-core/src/secretshape.rs:111`, `:2302` | `secrets` |
| S3 | 63 gitleaks-derived vendor shapes (each keyword-prefiltered) | `crates/purlis-core/src/secretshape.rs:1727` (`GITLEAKS`) | `secrets` |
| S4 | 12 personal-data rules (email, card, US SSN, IBAN, UK NINO, CA SIN, Aadhaar, Steuer-ID, DNI, codice fiscale, CPF, BSN), with check digits; email skips doc/no-reply/`git@` addresses | `crates/purlis-core/src/secretshape.rs:2144` (`pii_rules`), `:2330` | `guards.save` |
| S5 | Exemptions from the assignment rule: a vault reference (`vault:a/b`, names <= 32 chars), an UPPER_CASE variable name (<= 64 chars), a code expression | `crates/purlis-core/src/secretshape.rs:89`, `:368`, `:732` | `secrets` |
| S6 | Which rule set each surface applies: commit/push scan = keys + forge-token + gitleaks + PII, no assignment/JWT (`leaks` :2676); plane save, settings editors, handoff/dispatch brief, session record = full `secret_kind`/`kind_as_read` incl. assignment (:839, :1052); repo save + send to runner = keys + prefixes only, no JWT/assignment (`token_kind` :932) | `crates/purlis-core/src/secretshape.rs:2676`, `:839`, `:932` | `secrets` |
| S7 | Consumers that refuse/redact on a shape: handoff brief `crates/purlis-cli/src/handoff.rs:241`; dispatch brief `crates/purlis-cli/src/dispatch.rs:375`; app rebrief `app/src-tauri/src/rebrief.rs:205`; dispatch message dropped `crates/purlis-core/src/dispatchrecord.rs:738`; session record `crates/purlis-core/src/sessionrecord.rs:248`; settings saves `crates/purlis-core/src/settings.rs:526`, `settings/workspace.rs:628,740`, `settings/harness_profiles.rs:211`; git identity `doctor/fix/identity.rs:314`; plane save `planegit.rs:2959`; repo save `reposave.rs:1263`; artifact pin `artifact/mod.rs:341`; repo instructions `repoinstructions.rs:333`; profile send `profilestore/sent.rs:285`; redaction (report `report.rs:213`, app log `applog.rs:368`, model summary `harness/model.rs:366`, MCP error `secrets/mcpstart.rs:328`, share text `poolsetup/admin.rs:43`, project index `projectindex/words.rs:27`, asked edit `harness/asked/edit.rs:190`) | listed | `secrets` |

### forge

| # | Decision | Where | Family |
|---|---|---|---|
| 1 | A chat's (or MCP call's) forge call through purlis never gets the native transport or the person's purlis sign-in token; it always goes through the forge CLI (`gh`/`glab`) login | crates/purlis-core/src/forge/route.rs:60 | `guards.credential` |
| 2 | A chat gets no forge credential at all on a host whose CLI login the person imported into purlis | crates/purlis-core/src/forge/route.rs:62; crates/purlis-core/src/forge/backend.rs:1337 (`ChatsLoseAnImport`), text at backend.rs:1353 | `guards.credential` |
| 3 | A forge sign-in is handed over only by the host; `HostScope::claim` refuses inside a chat's process tree (env marks `CHAT_ENV`, session id) | crates/purlis-core/src/forge/route.rs:84 (`CHAT_MARKS`), :88 (`claim`) | `guards.credential` |
| 4 | Every chat/MCP forge request is admitted as *background*, and background requests are held back once the account's hourly allowance is spent | crates/purlis-core/src/forge/budget.rs:228 (`admission`), :57 (`HOURLY_ALLOWANCE` = 1000/h/account) | `limits` |
| 5 | Polling backs off when the forge's own remaining limit is below 20% | crates/purlis-core/src/forge/budget.rs:60 (`FLOOR_PERCENT`); crates/purlis-core/src/forge/poll.rs:40 (`BACK_OFF`=4), :42 (`PACED`) | `limits` |
| 6 | The CLI transport (the chat's route) never sends a file (`-F name=@path`) | crates/purlis-core/src/forge/cli.rs:250 (`FILE_REFUSED`) | `guards.forge` |
| 7 | MCP tool `purlis.forge_milestones`: only a repo of the chat's own workspace, only params `repo`/`state`, asked as `Caller::mcp(chat)`, at most 3 pages | crates/purlis-core/src/chattools.rs:1612; crates/purlis-core/src/forge/backend.rs:637 (`MILESTONE_PAGES`=3) | `guards.forge` |
| 8 | A persona tool named/whose program is `gh`/`glab`, run with a vault's value, refuses credential/settings commands (gh: auth, config, alias, extension, ssh-key, gpg-key, secret, repo deploy-key/set-default; glab: also token, deploy-key, variable, securefile, duo, mcp, opentofu init/download, cluster token…, rc token, ci --with-variables, repo view -F/--output/--jq, schedule --variable, orbit local), by every alias | crates/purlis-core/src/sandbox/tools/args.rs:183 (`Forge::refused`), called from crates/purlis-core/src/sandbox/tools.rs:2523 (`allows`) | `vaults` |
| 9 | Such a run must start with its command, not an option (only `--help`/`-h`/`--version`/`-v` alone are allowed) | crates/purlis-core/src/sandbox/tools/args.rs:263 (`ONLY_PRINTS`), :272 (`refused_forge`) | `vaults` |
| 10 | Such a run with `--` anywhere is refused (hands the rest to `git clone`) | crates/purlis-core/src/sandbox/tools/args.rs:291 | `vaults` |
| 11 | `gh api`/`glab api` with `--hostname` or a whole-URL endpoint is refused (only the forge's own host) | crates/purlis-core/src/sandbox/tools/args.rs:307 | `vaults` |
| 12 | `api` reaching a credential/secret route on the forge's own host is refused whatever the method (key, secret, token, runner-token, variable, terraform state, secure_files… routes; percent-decoded, `..` resolved, `/`-filling placeholders refused) | crates/purlis-core/src/sandbox/tools/args/api.rs:15 (`GH_ROUTES`), :56 (`GLAB_ROUTES`), :99 (`SPREADING`), :200 (`refused`) | `vaults` |
| 13 | `gh api graphql` refused where the query names a credential field (or is read from a file/stdin); `glab api graphql` refused whole | crates/purlis-core/src/sandbox/tools/args/api.rs:103 (`GH_GRAPHQL_FIELDS`), :222 | `vaults` |
| 14 | A persona tool's argument allowlist: every argument must match one `args` pattern (`*` glob; options only by a `-` pattern); absent = any argument | crates/purlis-core/src/sandbox/tools/args.rs:131 (`permits`) | `tools` |
| 15 | Limits on that allowlist: at most 64 patterns, each at most 256 bytes | crates/purlis-core/src/sandbox/tools/args.rs:35 (`MOST`), :38 (`LONGEST`) | `limits` |
| 16 | `mint = "github-app"` tool: the chat's `gh` gets a GitHub App installation token (in `GH_TOKEN`), never the App key | crates/purlis-core/src/sandbox/tools/mint.rs:32 (`GITHUB_APP`), :35 (`VAR`) | `guards.credential` |
| 17 | Minted token is limited to the github.com remotes of the clones the chat's workspace had at chat start (only clones whose git dir is in no grantable folder); GHES refused; one installation (the run's clone's, else the only one) | crates/purlis-core/src/forge/apptoken.rs:308 (`remotes_of_workspace`), :54 (`HOST`), :49 (`SEVERAL_INSTALLATIONS`) | `guards.credential` |
| 18 | Minted token bounds: at most 500 repos; kept in memory only, re-minted 10 min before expiry, kept at most 50 min; at most 256 kept; revoked when the chat ends | crates/purlis-core/src/forge/apptoken.rs:124 (`MOST_REPOS`), :133, :137, :140, :856 (`chat_ended`) | `guards.credential` |
| 19 | Agent identity App (made from the window) asks for contents/issues/pull_requests write + metadata read; workflows write only if the person ticks it; flow open 15 min | crates/purlis-core/src/forge/agentidentity.rs:131 (`permissions`), :103 (`workflows`), :56 (`ALLOWED`) | `guards.forge` |
| 20 | An App key in a vault under the reserved prefix is never handed out raw; only the mint reads it | crates/purlis-core/src/sandbox/tools/mint.rs:41 (`RESERVED_PREFIX`) | `vaults` |
| 21 | Unattended, a command that prints the forge token (`gh auth token`, `glab config get token`, `git credential fill`…) or reads a keychain/password-manager secret is refused | crates/purlis-core/src/floorguard/token.rs:40 (`shows_token`), :61 (`forge_reason`), :76 (`git_reason`), :104 (`keychain_read_reason`); caller crates/purlis-core/src/floorguard.rs:316 | `guards.release` |
| 22 | e2e build only: a forge call no recording answers is refused | app/src-tauri/src/e2eforge.rs:31 (`RECORDED_FORGE`), :103 | `guards.forge` |

### merge

| # | Decision | Where | Family |
|---|---|---|---|
| 1 | Merging is on the release floor: `gh pr merge`, `glab mr merge`/`accept`, `purlis/charter change land` are denied when the run is unattended | crates/purlis-core/src/floorguard.rs:78 (`PUBLISH_FORGE`), :196 (`MERGE_ROWS`), crates/purlis-core/src/floorguard/merge.rs:68 (`VERB_ALIASES`); attendance check crates/purlis-core/src/floorguard.rs:272 | `guards.merge` |
| 2 | In a chat the app started, a merge is refused to the chat and handed to the app as a merge ask under `enforce`; under `log` it is logged and under `off` it is not asked, by the hook and by the one-login forge route alike (`forge::mergestance`, ADR 0128 amended 2026-11-22) | crates/purlis-cli/src/guard.rs:217 (`held_merge`), :220 (only with a chat number), call at :147; precedence over A4 at crates/purlis-core/src/toolgate.rs:381 | `guards.merge` |
| 3 | A harness the app did not start gets no hold; only the floor, read from its own `permission_mode` | crates/purlis-core/src/toolgate.rs:377 | `guards.merge` |
| 4 | A merge whose request cannot be named (no number, raw API, two merges on a line) is refused with no ask | crates/purlis-core/src/mergehold.rs:213 (`held`) | `guards.merge` |
| 5 | `purlis change land` in an app-started chat is refused with no ask (the person's to do) | crates/purlis-core/src/mergehold.rs:217 | `guards.merge` |
| 6 | `gh/glab api` writes to a merge path, or with a field that speaks of merging (auto-merge), or with a shell-filled method/endpoint/key, or `..`, are refused as merges | crates/purlis-core/src/floorguard/merge.rs:447 (`api_reason`), :421 (`is_merge_path`) | `guards.merge` |
| 7 | GraphQL calls that may merge (or whose query the shell expands) are refused | crates/purlis-core/src/floorguard/merge.rs:544 (`graphql_reason`) | `guards.merge` |
| 8 | `glab mr create --auto-merge` is a merge | crates/purlis-core/src/floorguard/merge.rs:167 (`sets_auto_merge`) | `guards.merge` |
| 9 | A push option (`-o`, `push.pushOption` config, `GIT_CONFIG_*` env) that sets merge-when-ready is a merge | crates/purlis-core/src/floorguard/merge.rs:655, :788, :816 | `guards.merge` |
| 10 | `gh/glab alias set` standing for a held command is refused; `alias import` refused as unreadable | crates/purlis-core/src/floorguard/merge.rs:179, :155; held list :84 (`HELD_BEYOND_THE_TABLE`) | `guards.merge` |
| 11 | The hook waits 1.5 s for the app to take a merge, the app reads the forge within 1.2 s; no answer = refused, nobody asked | crates/purlis-cli/src/guard.rs:207 (`A_MERGE_IS_TAKEN_WITHIN`); crates/purlis-core/src/mergehold.rs:77 (`THE_FORGE_IS_READ_WITHIN`) | `guards.merge` |
| 12 | At most 3 merge asks waiting per chat | crates/purlis-core/src/mergehold.rs:80 (`AT_MOST_PER_CHAT`); refusal text app/src-tauri/src/mergeholds.rs:513 | `limits` |
| 13 | Merge asks share the chat's brokered-write rate | app/src-tauri/src/mergeholds.rs:452; crates/purlis-core/src/brokered.rs:1426 (`WRITES_A_MINUTE`=60) | `limits` |
| 14 | Only a repo the project inventory lists (same forge kind, path, host), from a clone in the plane, and the repo the command named must be that clone's | crates/purlis-core/src/mergehold.rs:595 (`listed_in`); app/src-tauri/src/mergeholds.rs:260 (`found`) | `guards.merge` |
| 15 | The chat must be one this app has open; the same merge already waiting on another forge is refused | app/src-tauri/src/mergeholds.rs:449, :469 | `guards.merge` |
| 16 | No grant/rule is ever remembered from a merge answer | crates/purlis-core/src/askrules.rs:338 | `guards.merge` |
| 17 | Only the window presses Merge / Don't merge (commands are window-only; answerer is the local UI) | crates/session-protocol/src/ui.rs:406 (in `WINDOW_ONLY`, :203); app/src-tauri/src/mergeholds.rs:534 (`the_window`), :704, :871 | `guards.merge` |
| 18 | A merge takes two presses | crates/purlis-core/src/mergehold/looked.rs:24 (`PRESSES`=2) | `guards.merge` |
| 19 | A press merges only at the head asked, with the same base, same ruleset state shown, same facts digest shown; otherwise nothing merges and the ask waits again | crates/purlis-core/src/mergehold.rs:1123 (`press`), :1141, :1152; app/src-tauri/src/mergeholds.rs:774, :779, :783 | `guards.merge` |
| 20 | A press merges by the repo's own method, through the merge queue/train when there is one; a forge that would only "merge later" (auto-merge) is an error, not a merge | crates/purlis-core/src/mergehold.rs:665, :682 | `guards.merge` |
| 21 | A merge ask's facts (title, author, checks, ruleset, approvals) come from the forge with the app's own login, never the chat; one not read in time is "unknown", never passed, and the ask is still raised | crates/purlis-core/src/mergehold.rs:474 (`read`), :532 | `guards.merge` |
| 22 | While an ask waits the app re-reads the forge every 30 s; a moved head retires the ask | app/src-tauri/src/mergeholds.rs:310 (`WATCH_EVERY`); crates/purlis-core/src/mergehold.rs:1061 | `guards.merge` |
| 23 | View changes shows at most 1 MiB of the diff; a cut diff does not mark the head viewed | app/src-tauri/src/mergeholds.rs:230 (`CHANGES_SHOWN`) | `guards.merge` |
| 24 | Viewed heads and recorded pushes kept: at most 256 | crates/purlis-core/src/mergehold/looked.rs:27 (`AT_MOST`) | `guards.merge` |
| 25 | Merge asks live in memory only; a relaunch drops them | crates/purlis-core/src/mergehold.rs:49 | `guards.merge` |
| 26 | A `pr-merge` save is merged by purlis itself only when every check at the pushed head passed (none past failing/pending/required review); with no checks, only after 60 s settle; never asks the forge to auto-merge | crates/purlis-core/src/forge/pr.rs:327 (`Gate`), :340 (`SETTLE`) | `guards.merge` |
| 27 | `change land` gates: one member, not by rebase; member open; its `needs` landed; checks PASSED at the exact head; merge pinned to that head, via queue/train where present | crates/purlis-core/src/change/land.rs:119 (`land`) | `guards.merge` |

### release

| # | Decision | Where | Family |
|---|---|---|---|
| 1 | An unattended run may not publish (release floor, arm A4); attended runs are untouched | crates/purlis-core/src/toolgate.rs:369 (`release_floor`); crates/purlis-core/src/floorguard.rs:272 | `guards.release` |
| 2 | `gh release create` (alias `new`) and `glab release create` are publishes | crates/purlis-core/src/floorguard.rs:78 (`PUBLISH_FORGE`); crates/purlis-core/src/floorguard/merge.rs:68 | `guards.release` |
| 3 | Creating a git tag is a publish's first step; only list/delete/verify flags are harmless | crates/purlis-core/src/floorguard.rs:106 (`TAG_HARMLESS`), :347 | `guards.release` |
| 4 | `git push --tags` / `--follow-tags` | crates/purlis-core/src/floorguard.rs:352 | `guards.release` |
| 5 | `git push` naming a `refs/tags/` ref | crates/purlis-core/src/floorguard.rs:356 | `guards.release` |
| 6 | `git push` naming something shaped like a version tag (`v?\d+.\d+…`) | crates/purlis-core/src/floorguard.rs:145 (`version_tag_re`), :360 | `guards.release` |
| 7 | `gh/glab api` writes creating a release, release asset, tag or `refs/tags/` ref (or an unreadable new ref) | crates/purlis-core/src/floorguard/merge.rs:507 (`publish_reason`) | `guards.release` |
| 8 | A git alias (on the line or written via `git config alias.*`) standing for `push`/`tag` or anything the floor refuses | crates/purlis-core/src/floorguard/merge.rs:785 (`HELD_GIT`), :788 | `guards.release` |
| 9 | `purlis skill publish` is on the floor unattended, and refused outright from any chat or without a person at a terminal | crates/purlis-core/src/floorguard.rs:89 (`PUBLISH_SINCE`); crates/purlis-cli/src/skill.rs:3264 (`may_publish`) | `guards.release` |
| 10 | Publishing a local-only project to GitHub/GitLab is a window-only action no chat reaches; repo private unless the person says public | crates/purlis-core/src/publishproject.rs:155 (`publish`; rule in module doc :33) | `consents` |
| 11 | What the floor cannot read is refused: more than 6x the line's length + 64 KiB read across nesting levels, or nested deeper than 16 | crates/purlis-core/src/floorguard.rs:229 (`WORK_FACTOR`), :233 (`WORK_SLACK`), :268 (`MAX_DEPTH`) -> crates/purlis-core/src/guardcaps.rs:63 (`MAX_COMMAND_DEPTH`=16) | `guards.release` |

### save

| # | Decision | Where | Family |
|---|---|---|---|
| S1 | How far a plane save goes: off / commit / push / pr / pr-merge | crates/purlis-core/src/planesave.rs:135 (`pick ["plane"] "mode"`); off handled at crates/purlis-core/src/planegit.rs:3282 | `guards.save` |
| S2 | How far a workspace repo save goes | crates/purlis-core/src/planesave.rs:234-242 (`Mode::Off` default) | `guards.save` |
| S3 | Whether the app saves the plane by itself | crates/purlis-core/src/planesave.rs:161; gate crates/purlis-core/src/autosave.rs:36 | `guards.save` |
| S4 | Whether the app saves a repo by itself | crates/purlis-core/src/planesave.rs:246; gate crates/purlis-core/src/autosave.rs:42 | `guards.save` |
| S5 | Quiet period before an auto-save | crates/purlis-core/src/planesave.rs:257 (`QUIET`), read at :162/:247 | `guards.save` |
| S6 | Auto-save also fires at a chat's end, at app quit and at launch (push what was left) | app/src-tauri/src/autosave.rs:9-15 (worker doc), `at_quit` app/src-tauri/src/autosave.rs:739 | `guards.save` |
| S7 | Bound on pushes at quit | app/src-tauri/src/autosave.rs:57 (`QUIT_BOUND`) | `guards.save` |
| S8 | Retry after a save that did not settle (remote down) | crates/purlis-core/src/autosave.rs:25 (`RETRY_AFTER`) | `guards.save` |
| S9 | Auto-save's look/fetch cadence; fast-forward a clean tree onto the target only while auto-save is on | app/src-tauri/src/autosave.rs:51 (`LOOK_EVERY` 2 s), :53 (`FETCH_EVERY` 300 s), :55 (`FETCH_AT_MOST_EVERY` 60 s), :60 | `guards.save` |
| S10 | pr-merge: purlis merges its own save PR only once every check passed (never at save time, never past a red check, required or not); forge auto-merge never asked | crates/purlis-core/src/forge/pr.rs:327 (`Gate::Checks`), :340 (`SETTLE` 60 s); reposave.rs:16-21 doc | `guards.save` |
| S11 | Looks at an open pr-merge request: backoff and give-up | crates/purlis-core/src/autosave.rs:326 (`LOOK_MARGIN` 5 s), :328 (`LOOK_AT_MOST` 300 s), :332 (`LOOK_FOR` 2 h), :334 (`LOOK_AT_LEAST` 60 s) | `guards.save` |
| S12 | Merge a save PR "as it stands" (failed/missing checks included) only by the person's press in the window, re-checked against what was shown | crates/purlis-core/src/reposave.rs:1041 (`merge_as_seen`), :1076; crates/purlis-core/src/forge/pr.rs:335 (`Gate::Person`); window-only crates/session-protocol/src/ui.rs:732 (`merge_save_request`) | `consents` |
| S13 | A request-mode repo save never pushes to the default branch or the PR base; makes `purlis/<workspace>/<short-sha>` instead | crates/purlis-core/src/reposave.rs:557 (`protected`), :1122 (`save_branch`), :858 | `guards.save` |
| S14 | A repo push never rebases/rewrites history; a refused push leaves it blocked | crates/purlis-core/src/reposave.rs:30-33 doc (no rebase path in `commit_push` :252) | `guards.save` |
| S15 | A PR from the same branch that purlis did not open keeps its title, body and merging | crates/purlis-core/src/reposave.rs:732 (`!opened.ours`) | `guards.save` |
| S16 | Plane save's target branch and rolling save branch | crates/purlis-core/src/planesave.rs:158-159; default name crates/purlis-core/src/planesave.rs:77-90 (`purlis/save/<host>-<6hex>`) | `guards.save` |
| S17 | Whether save commits are signed | crates/purlis-core/src/planesave.rs:160 (plane), :245 (repo) | `guards.save` |
| S18 | A save made for a chat is never signed, whatever `--sign` or `[plane] sign` says | crates/purlis-core/src/planegit.rs:3628-3629 (`sign = asked_to_sign && !request.for_a_chat`) | `guards.save` |
| S19 | How `Assisted-by` is spelled on an agent's save | crates/purlis-core/src/planesave.rs:163, repo :249-251 (falls back to plane) | `guards.save` |
| S20 | Auto-save commits carry no provenance trailers (app's commit) | crates/purlis-core/src/autosave.rs:7-16 doc | `guards.save` |
| S21 | `purlis worktree commit` refuses a chat with no sandbox and points it at git | crates/purlis-core/src/planesave.rs:164-169 (`refuse_unsandboxed_commit`), used crates/purlis-core/src/gitbroker/commit.rs:95 | `guards.git` |
| S22 | A chat's `purlis save` is done by the app (the project's git folder is the app's to write) when an app takes it; else it runs in the chat's process | crates/purlis-cli/src/main.rs:3173-3183 (`GitWork::RootSave` forwarded); crates/purlis-core/src/gitbroker.rs:449 | `guards.plane` |
| S23 | Time the CLI waits for the app to save for a chat | crates/purlis-cli/src/gitask.rs:132 (`A_SAVE_TAKES_AT_MOST`) | `limits` |
| S24 | A save for a chat refuses `--pull` | crates/purlis-core/src/gitbroker.rs:457 (`SAVE_PULL_FOR_A_CHAT` :816) | `guards.save` |
| S25 | A save for a chat leaves out what the chat's sandbox may not read | crates/purlis-core/src/gitbroker.rs:468 → crates/purlis-core/src/planegit.rs:3080 (`save_keeping_out`) | `sandbox` |
| S26 | A save for a chat drops trailer-shaped lines from the chat's message; trailers are the app's | crates/purlis-core/src/gitbroker.rs:467 (`without_trailers`) | `guards.save` |
| S27 | Plane save stages the whole tree except local skills, `skills.local.lock`, throwaway pieces and the resolve folder | crates/purlis-core/src/planegit.rs:2537 (`never_staged`), :2556 (`save_stages`) | `guards.save` |
| S28 | Refuse to stage all of the plane from inside a linked worktree of it | crates/purlis-core/src/planegit.rs:3634 | `guards.save` |
| S29 | Plane save's secret guard: refuse when any staged blob looks like a secret | crates/purlis-core/src/planegit.rs:3888 (guard), :2958 (`secret_in` → secretshape), :760 (`SECRET_REFUSED`); window shows it app/src-tauri/src/blockedsave.rs | `guards.credential` |
| S30 | Repo save's narrower secret rule: credential-named files (`.env*` but not example/sample/template/dist, SSH keys, `*.pem/.p12/.pfx/.key`, `credentials.json`, `.npmrc`/`.pypirc` with token) or a private key / forge token in staged text | crates/purlis-core/src/reposave.rs:1196 (`secret_name`), :1249 (`refused_as`), :1233, used :395 | `guards.credential` |
| S31 | A repo save waits while any chat in its workspace is mid-turn: a manual/CLI save is refused, an auto-save skips, a quit save goes ahead/skips | crates/purlis-core/src/reposave.rs:125-131, re-asked :302 | `guards.save` |
| S32 | One save of a tree at a time | crates/purlis-core/src/planegit.rs:3062 (`Claim::of`, `ALREADY_SAVING` :2997); reposave.rs:132 | `guards.save` |
| S33 | A project purlis may not write (read-only/compat) or whose git files are refused is never saved, whoever asks | crates/purlis-core/src/planegit.rs:3274, :3278; :3216 (`paths_unsaved`) | `guards.save` |
| S34 | Git write timeout for a plane save (commit/rebase) | crates/purlis-core/src/planegit.rs:1830 (`WRITE`) | `limits` |
| S35 | Recognised "protected branch" push rejections (drive the request-mode path) | crates/purlis-core/src/planegit.rs:1705 (`PROTECTED_SIGNATURES`) | `guards.save` |
| S36 | Save, merge-as-it-stands, Fix, settle conflict, Update from remote, Open blocked file are the person's presses only | crates/session-protocol/src/ui.rs:623-625, :728, :732, :735, :740-745, :756 (`WINDOW_ONLY`) | `consents` |
| S37 | Who the forge is asked as for a save (window = person's sign-in; command; trigger = none) | crates/purlis-core/src/planegit.rs:3028 (`Trigger::caller`) | `guards.save` |

### kill switch

| # | Decision | Where | Family |
|---|---|---|---|
| K1 | Stopped on disk if the `halted` marker exists or the journal's last entry is a stop, under any config-home spelling | crates/purlis-core/src/halt.rs:150 (`stopped_on_disk`), :32 (`FILE`), :35 (`JOURNAL`) | `fixed` |
| K2 | Throwing the switch ends every chat and shell purlis started, in every project/window, and every browser | app/src-tauri/src/killswitch.rs:585 (`stop_every_agent`), :591 (browsers); app/src-tauri/src/planes.rs:5635; host: crates/host/src/hosting.rs:2258 (`stop_all`) | `fixed` |
| K3 | While thrown, every chat/harness start is refused (new chat, relaunch, handoff, dispatch, curation action), except a shell the operator opens from the window | crates/host/src/hosting.rs:2244 (`let_through`); app/src-tauri/src/sessions.rs:162, :281; app/src-tauri/src/hostedsessions.rs:4744, :4898, :4948 | `fixed` |
| K4 | Only the window re-arms; `purlis stop --all` never re-arms; host re-arm only from the window's link | app/src-tauri/src/killswitch.rs:296 (`rearm`); crates/session-protocol/src/ui.rs:592 (`rearm_agents` WINDOW_ONLY); crates/host/src/hosting.rs:4265 | `fixed` |
| K5 | In-memory switch is the authority while the app runs; disk can only move it towards stopped; a removed marker is put back and journalled as tamper | app/src-tauri/src/killswitch.rs:329 (`look`), :340 (`halt::restore`) | `fixed` |
| K6 | Re-arm refused (agents stay stopped) if the session host or the files did not take it | app/src-tauri/src/killswitch.rs:297-321 | `fixed` |
| K7 | Watch cadence / how fast a CLI stop is heard | app/src-tauri/src/killswitch.rs:53 (`FOLDED_FOR` 100 ms), :57 (`MOST_PATHS` 256), :61 (`LOOK_EVERY` 3 s), :70 (`HEARD_WITHIN` 30 s) | `fixed` |
| K8 | Journal kept to N lines | crates/purlis-core/src/halt.rs:38 (`JOURNAL_LINES`) | `fixed` |
| K9 | A runner (remote session host) heeds `purlis stop --all` with no window | crates/purlis-cli/src/main.rs:1251 (`heeding(halt::OnDisk)`) | `fixed` |

### plumbing (guard command, caps, seen)

| # | Decision | Where | Family |
|---|---|---|---|
| P1 | Which tool calls the guard hooks see: `pretooluse` (Bash), `pretooluse-read` (Read\|Grep), `pretooluse-edit` (Write\|Edit\|MultiEdit), `pretooluse-dispatch` (Task\|Agent\|mcp__purlis__dispatch*) | crates/purlis-core/src/hookreg.rs:90-117 | `guards.shape` |
| P2 | Harness-side hook timeouts | crates/purlis-core/src/hookreg.rs:93 (Bash 10 s), :99/:105/:116 (5 s) | `limits` |
| P3 | Guard decides within its own budget; past it the call is refused (never left to the harness timeout) | crates/purlis-cli/src/guard.rs:359 (`BUDGET`), :413 (`decided_within`), :405 (`out_of_time`); crates/purlis-cli/src/main.rs:2677 (payload read inside the budget) | `guards.shape` |
| P4 | A crashed / panicked PreToolUse hook refuses the call (exit 2), not allow | crates/purlis-cli/src/guard.rs:454 (`refuse_on_a_crash`); crates/purlis-cli/src/main.rs:2274 | `guards.shape` |
| P5 | A decided denial that cannot be printed falls back to exit 2 | crates/purlis-cli/src/guard.rs:496 (`deny`), :58 (`DENY_EXIT`) | `guards.shape` |
| P6 | Guard runs on a large stack so overflow cannot become an allow | crates/purlis-cli/src/guard.rs:342 (`GUARD_STACK`) | `guards.shape` |
| P7 | An unknown hook word in the `pretooluse`/`posttooluse` namespace blocks; any other unknown word exits 1 (non-blocking) | crates/purlis-cli/src/main.rs:2266 (`is_a_tool_hook`), :2744 | `guards.shape` |
| P8 | A payload that will not parse is `{}`: every arm sees an empty command and none fires | crates/purlis-cli/src/guard.rs:73-76 | `guards.shape` |
| P9 | Plane-gated arms run only where `charter.toml` exists at the resolved root (`$CHARTER_ROOT` alone does not count); the leak guard is ungated | crates/purlis-cli/src/guard.rs:106 (`has_manifest`) | `guards.shape` |
| P10 | Order of a Bash call's checks: bookkeeping → toolgate refusals → merge hold → branch-folder claim → persona allow | crates/purlis-cli/src/guard.rs:140, :147, :151-156, :164 | `guards.shape` |
| P11 | Caps before any guard reads a command: size, substitution nesting, wrapper layers; past one → refused `too-big-to-check` | crates/purlis-core/src/guardcaps.rs:47 (`MAX_COMMAND_BYTES` 128 KiB), :54 (`MAX_NESTING` 8), :59 (`MAX_LAYERS` 64), :120 (`refusal`); applied crates/purlis-core/src/toolgate.rs:399 | `guards.shape` |
| P12 | How deep the release floor reads a command inside a command | crates/purlis-core/src/guardcaps.rs:63 (`MAX_COMMAND_DEPTH` 16) → crates/purlis-core/src/floorguard.rs:268 | `guards.shape` |
| P13 | A reading nested too deep fails closed, also for the persona allow | crates/purlis-cli/src/guard.rs:166 (`too_deep`); crates/purlis-core/src/guardcaps.rs:111 | `guards.shape` |
| P14 | Merge hold asked only of app-started chats; no app answer = refused anyway; wait for the app | crates/purlis-cli/src/guard.rs:217 (`held_merge`), :220, :207 (`A_MERGE_IS_TAKEN_WITHIN` 1.5 s), :235 (unreadable/land → no ask) | `guards.merge` |
| P15 | Branch-folder one-writer guard: only app-started, unsandboxed chats; an unplaceable git write is refused; otherwise no app answer lets git run | crates/purlis-cli/src/guard.rs:269, :153, :278, :254 (`CLAIMS_ARE_ANSWERED_WITHIN` 900 ms) | `guards.git` |
| P16 | Attendance read from the app; no answer in time = unattended | crates/purlis-cli/src/guard.rs:182, :175 (`ATTENDANCE_IS_ANSWERED_WITHIN` 500 ms) | `mode` |
| P17 | Persona tool gate: `allow` (skip the harness prompt) when the active persona's `tools:` declares the program; never denies; declines for interpreters/wrappers, destructive subcommands, control-surface paths | crates/purlis-core/src/toolhooks.rs:726 (`persona_allow`); crates/purlis-core/src/personagate.rs:850 (`decide`), :117 (`INTERPRETERS`), :367 (`dangerous`) | `tools` |
| P18 | `purlis guard ask | allow` writes the rule into every harness's own file, all or nothing | `tools` |
| P19 | An `allow` rule reaches the plane root only; workspace/clone layers carry `ask` and `deny`, never `allow`; an `ask` rule is mirrored into every workspace layer at once | crates/purlis-core/src/layer.rs:155 (`RESTRICTIVE`); crates/purlis-core/src/guardcmd.rs:364 (`mirror`) | `tools` |
| P20 | Which rule shapes can be written (Claude tools list, exact MCP names only, else `Bash(...)`) | crates/purlis-core/src/guardcmd.rs:58 (`RULE_TOOLS`), :92 (`as_rule`) | `tools` |
| P21 | A settings file reached through a link is never written | crates/purlis-core/src/guardcmd.rs:167 | `tools` |
| P22 | Consent ask rules written by `purlis init`/`reinit`: `report --yes`, todo promote; handoff rule retired | crates/purlis-core/src/scaffold/settings.rs:70 (`CONSENT_PATTERNS`), :51, :61; retired crates/purlis-core/src/guardcmd.rs:41 | `consents` |
| P23 | Guard sighting: last time and harness the Bash guard ran, for doctor (observation, not a decision) | crates/purlis-core/src/guardseen.rs:30 (`guard-seen.json`), :92 (`mark`) | `guards.shape` |

### shell guard

| # | Decision | Where | Family |
|---|---|---|---|
| 1 | Which harnesses get a shim in a shell tab | `crates/purlis-core/src/shellguard.rs:56` (`SHIMMED`); mirrors `crates/purlis-core/src/harness.rs:107` (`Harness::ALL`) | `guards.shape` |
| 2 | A harness started by hand is never blocked: purlis prints a warning, tells the app, then `exec`s the real program | `crates/purlis-core/src/shellguard.rs:383` (`plan`); `crates/purlis-cli/src/shellguard.rs:18` (`run`) | `guards.shape` |
| 3 | The app is told (banner with "Open as chat") only when the hook socket and a chat number above 0 are both in the environment | `crates/purlis-core/src/shellguard.rs:391` | `guards.shape` |
| 4 | After the person's own rc files run, the shim directory goes first on `PATH` again, for zsh (`ZDOTDIR`) and bash (`--rcfile`) only. Other shells get `PATH` alone, so their rc files can put a harness ahead of the shim | `crates/purlis-core/src/shellguard.rs:194` | `guards.shape` |
| 5 | The shim directory is owner-only (0700) | `crates/purlis-core/src/shellguard.rs:123` | `guards.shape` |
| 6 | No shims where they cannot be written (non-unix, or a write error), so the shell tab is a plain shell with no warning | `app/src-tauri/src/lib.rs:297` (`shell_tab_shims`) | `guards.shape` |
| 7 | No real program on `PATH` outside the shims gives exit 127; a failed `exec` gives 126 | `crates/purlis-cli/src/shellguard.rs:36`, `:56` | `guards.shape` |
| 8 | How a Bash command line is split into commands: punctuation `();<>\|&` plus newline separate commands; whitespace is ` \t\r` | `crates/purlis-core/src/shellseg.rs:77`, `:80`, `:137` | `guards.shape` |
| 9 | A line that will not lex falls back to a cruder segmentation and is flagged as "not a parse", so callers also match the raw string (fail closed) | `crates/purlis-core/src/shellseg.rs:1481` (`read`), `:1410` (`fallback_segments`) | `guards.shape` |
| 10 | Substitution nesting limit: a reading deeper than 8 levels counts as "too deep" and the call is refused | `crates/purlis-core/src/shellseg.rs:1188`, `:1053` (`too_deep_within`); constant in `crates/purlis-core/src/guardcaps.rs:54` (`MAX_NESTING = 8`) | `guards.shape` |

### tool gate

| # | Decision | Where | Family |
|---|---|---|---|
| 1 | Every Bash call goes through the guard before it runs: matcher `Bash`, 10 s hook timeout | `crates/purlis-core/src/hookreg.rs:90` (timeout `:93`); Codex config `crates/purlis-core/src/plugin_install.rs:1229`, `:1234`; opencode `crates/purlis-core/src/opencode.rs:137` | `guards.shape` |
| 2 | On opencode, every tool without its own hook word (MCP tools included) is routed to the Bash guard; a tool with no `command` is allowed | `crates/purlis-core/src/opencode.rs:195` (`DEFAULT_PRE`) | `guards.shape` |
| 3 | Fail closed: an unknown `pretooluse*`/`posttooluse*` hook word answers block (exit 2), and a crash in a PreToolUse hook refuses the call | `crates/purlis-cli/src/main.rs:2266` (`is_a_tool_hook`), `:2290` (`BLOCK = 2`); `crates/purlis-cli/src/guard.rs:454` (`refuse_on_a_crash`) | `guards.shape` |
| 4 | No override exists: no config key, environment variable or switch lifts a denial; the only way past is the person running the command themselves | `crates/purlis-core/src/toolgate.rs:75` (`OVERRIDE_NOTE`) | `guards.shape` |
| 5 | Retry-shaped arms (rider, forge/charter substitution, dispatch text, hook skip, handoff brief, branch git) name the shape to retry and drop the override note | `crates/purlis-core/src/toolgate.rs:92` (`RETRY_SHAPED`) | `guards.shape` |
| 6 | The arm order is fixed (first refusal wins): caps, A, A2, A3, A3b, A3c, A3d, A4, A5, A6, A7, A7d, A7e, A7b, A7c, A8, A9, A9b, A10 | `crates/purlis-core/src/toolgate.rs:236` (`arms!`), `:424` (`fn arms`) | `guards.shape` |
| 7 | Caps come before every arm: a command over 128 KiB, nested deeper than 8, run through more than 64 wrapper layers, or nested more than 16 commands deep is refused as `too-big-to-check` | `crates/purlis-core/src/toolgate.rs:396` (`verdict`); `crates/purlis-core/src/guardcaps.rs:47`, `:54`, `:59`, `:63` | `guards.shape` |
| 8 | Plane gate: A2, A3, A3b, A3c, A3d, A4, A7, A7b, A7c, A7d, A7e, A9, A9b and A10 run only inside a plane; A, A5, A6 and A8 run in any directory | `crates/purlis-core/src/toolgate.rs:424` (`plane: Option<&Plane>`) | `guards.shape` |
| 9 | A (leak guard) comes first and is never switchable | `crates/purlis-core/src/toolgate.rs:429` | `secrets` |
| 10 | A2: one credential per forge (no SSH, no signing) | `crates/purlis-core/src/toolgate.rs:443` (calls `credguard`) | `guards.credential` |
| 11 | A3: no branch move (`checkout`/`switch`, branch-creating and detach options) in the plane root's shared working tree | `crates/purlis-core/src/planeroot.rs:3189`; lists `:102` (`BRANCH_MOVERS`), `:107`, `:118`; alias hops `:244` (`MAX_ALIAS_HOPS = 4`), checkout operands `:154` (32) | `guards.plane` |
| 12 | A3b: no `git reset --hard/--merge/--keep` in the plane root that would destroy commits no remote has | `crates/purlis-core/src/planeroot.rs:3630`; `:159` (`RESET_TREE_MODES`) | `guards.plane` |
| 13 | A3c: in a sandboxed chat that may write the root, refuse git that would rewrite the root's working tree and that the sandbox would stop partway | `crates/purlis-core/src/rootupdate.rs:585` (`refusal_of`); gated at `crates/purlis-core/src/toolgate.rs:486` (`launched.sandboxed`) | `sandbox` |
| 14 | A3d: in a sandboxed chat, refuse a merge/rebase/pick/clone in a clone or branch folder that would write files the sandbox denies | `crates/purlis-core/src/cloneupdate.rs:1885`; flag lists `:59`, `:91`, `:122`, `:142`; `:178` (`MAX_ASKED_PATHS = 64`) | `sandbox` |
| 15 | A4: release floor | `crates/purlis-core/src/toolgate.rs:369` (`release_floor`) | `guards.release` |
| 16 | A5/A6: live substitution in published or persisted prose | `crates/purlis-core/src/toolgate.rs` (calls `proseguard`) | `guards.release` |
| 17 | A7: handoff | `crates/purlis-core/src/toolgate.rs:564` | `guards.shape` |
| 18 | A7d: a helper sub-agent may not dispatch (Bash `purlis dispatch …`, or the 12 MCP dispatch tools) | `crates/purlis-core/src/dispatchguard.rs:190` (`refusal`), `:236` (`tool_refusal`), `:211` (`OUR_TOOLS`) | `guards.shape` |
| 19 | A7e: the text of a dispatch subcommand (report/tell/answer/pass-on/note/ask) may not sit beside a live substitution | `crates/purlis-core/src/dispatchguard.rs:364`; `:284` (`SENDS_TEXT`) | `guards.shape` |
| 20 | A7b: a consent-gated command (`report --yes`, `todo promote`, and handoff placement) spelt in a way the project's consent rules do not match is refused | `crates/purlis-core/src/consentspelling.rs:129`, `:81` (`GUARDED`); patterns `crates/purlis-core/src/scaffold/settings.rs:44`, `:51`, `:61`, `:70`, `:83`, `:89` | `guards.shape` |
| 21 | A7c: a program named by one of the operator's own ask/deny rules, spelt so the host's glob would miss it, is refused | `crates/purlis-core/src/rulespelling.rs:111`; files read at `:471` (`.claude/settings.json`, `.claude/settings.local.json`, layer settings, `opencode.json`) and `:513` (user `~/.claude/settings.json` / `$CLAUDE_CONFIG_DIR`); `:316` (`MAX_STARTS_TRIED = 16`) | `fixed` |
| 22 | A8: a commit that skips hooks | `crates/purlis-core/src/toolgate.rs:627` (calls `commitguard`) | `guards.save` |
| 23 | A9: in a sandboxed chat outside the root, refuse git that takes one of git's lock files on the project (18 subcommands) | `crates/purlis-core/src/projectgit.rs:103`; `:43` (`LOCKING`) | `sandbox` |
| 24 | A9b: `git add`/`git commit` in a branch folder from a sandboxed chat; the retry is `purlis worktree commit` | `crates/purlis-core/src/toolgate.rs:657` (calls `branchgit`) | `sandbox` |
| 25 | A10: a dispatch, handoff or `worktree commit/update/remove/prune` sharing its call with another command (a "rider") is refused; applies only where Claude Code holds the pre-allow | `crates/purlis-core/src/dispatchguard.rs:819` (`rider_refusal`); `:450` (`CLAUDE_CODE`) | `guards.shape` |

### prose guard

| # | Decision | Where | Family |
|---|---|---|---|
| 1 | A5: a forge command that publishes prose may not have a live `$( )`/backtick/process substitution anywhere in the call | `crates/purlis-core/src/proseguard.rs:570` (`forge_substitution_hit`); shell judgement `crates/purlis-core/src/livesub.rs:294` | `guards.release` |
| 2 | A5's list: 19 `gh`/`glab` (noun, verb) pairs: issue create/comment/edit, pr create/comment/edit/review, release create/edit, gist create/edit; glab issue create/note/update, mr create/note/update, release create, snippet create | `crates/purlis-core/src/proseguard.rs:72` (`FORGE_PROSE`) | `guards.release` |
| 3 | A5's conditional rows, refused only when a text flag is given: gh issue/pr close/reopen `--comment`, gh project item-create/item-edit, gh agent-task create, glab incident note, glab work-items create/update | `crates/purlis-core/src/proseguard.rs:142` (`FORGE_PROSE_WHEN`) | `guards.release` |
| 4 | Deliberately not guarded: merge commit message (`gh pr merge --body`, `glab mr merge --message`), repo/label/milestone/project attributes, `secret set`/`variable set` values, `gh api`/`glab api`, glab close/reopen; also `git commit -m "$(cat <<'EOF')"` and `--body-file -` with a quoted heredoc | `crates/purlis-core/src/proseguard.rs:206` (`FORGE_TEXT_NOT_ROWS`) | `guards.release` |
| 5 | A6: purlis's own text-writing commands may not carry a live substitution. The 11 rows are persona remember/log, workspace remember/note/todo/vision, worktree abandon, change create/drop, report bug/gap (`--from-file`) | `crates/purlis-core/src/proseguard.rs:633` (`charter_substitution_hit`), `:288` (`CHARTER_PROSE_ROWS`); aliases `:267` (ws, wt), `:273` (gap → feature) | `guards.save` |
| 6 | A6 runs after A5, so a line that is both is explained as a forge publish | `crates/purlis-core/src/toolgate.rs:424` (order in `arms`) | `guards.release` |

### floor guard

| # | Decision | Where | Family |
|---|---|---|---|
| 1 | An unattended chat is denied a publish (deny, never ask); an attended chat is not checked | `crates/purlis-core/src/floorguard.rs:161` (`release_floor_reason`), `:272` (`if !unattended`) | `guards.release` |
| 2 | How attendance is read. A chat the app started uses the app's attendance fact; on Claude Code only, a line's `bypassPermissions` lowers it to unattended; on Codex the fact alone counts (Codex reports bypass on every line); a harness the app did not start uses `permission_mode == bypassPermissions` only | `crates/purlis-core/src/attendance.rs:181` (`of_a_line`), `:158` (`PROMPTS_OFF`); `crates/purlis-core/src/toolgate.rs:369`; `crates/purlis-core/src/harness/claude.rs:409` | `mode` |
| 3 | If the app does not answer the attendance ask within 500 ms (or answers anything but attended), the chat counts as unattended | `crates/purlis-cli/src/guard.rs:175` (`ATTENDANCE_IS_ANSWERED_WITHIN`), `:199` | `mode` |
| 4 | Forge publish verbs refused: gh release create, gh pr merge, glab release create, glab mr merge, `purlis change land`; `gh pr create` is deliberately absent | `crates/purlis-core/src/floorguard.rs:78` (`PUBLISH_FORGE`) | `guards.release` |
| 5 | `purlis skill publish` is refused unattended | `crates/purlis-core/src/floorguard.rs:89` (`PUBLISH_SINCE`) | `guards.release` |
| 6 | Verb aliases count as the same verb: `glab mr accept`, `gh release new` | `crates/purlis-core/src/floorguard/merge.rs:68` (`VERB_ALIASES`) | `guards.release` |
| 7 | Read beyond the table: `gh`/`glab` api, alias, auth, config get, and `glab mr create/new` (auto-merge) | `crates/purlis-core/src/floorguard/merge.rs:84` (`HELD_BEYOND_THE_TABLE`) | `guards.release` |
| 8 | `git tag <name>` (creating a tag) is refused unattended; the read-only/local flags in `TAG_HARMLESS` (-l, -d, --contains, -v …) are allowed | `crates/purlis-core/src/floorguard.rs:336`; `:106` (`TAG_HARMLESS`, 11) | `guards.release` |
| 9 | `git push` with `--tags`/`--follow-tags`, a `refs/tags/` ref, or a version-shaped word (`v?\d+.\d+…`) is refused unattended | `crates/purlis-core/src/floorguard.rs:352`; regex `:145` | `guards.release` |
| 10 | Auto-merge by other routes: a push option that speaks of merging, `git config push.pushOption`/`alias.*` that would publish, `GIT_CONFIG_*` env carrying pushoption | `crates/purlis-core/src/floorguard/merge.rs:655`, `:788`, `:816`; `:785` (`HELD_GIT` push, tag) | `guards.release` |
| 11 | Raw `gh api`/`glab api` writes to release, tag, ref or merge endpoints, GraphQL merge mutations, and unreadable methods or endpoints | `crates/purlis-core/src/floorguard/merge.rs:447` (`api_reason`), `:507`, `:544` | `guards.release` |
| 12 | Printing the forge token is refused unattended: `gh`/`glab auth token`, `auth git-credential`, `auth docker-helper`, `auth status --show-token`/`-t`, and `config get` of a key containing "token" | `crates/purlis-core/src/floorguard/token.rs:61` | `guards.release` |
| 13 | `git credential fill`, and a credential helper asked to `get`, are refused unattended | `crates/purlis-core/src/floorguard/token.rs:76` | `guards.release` |
| 14 | Reading the OS or password-manager store is refused unattended: `security find-*-password -w/-g`, `dump-keychain`; `secret-tool lookup/search`; `pass`/`gopass` except the quiet subcommands | `crates/purlis-core/src/floorguard/token.rs:104`; `:96` (`PASS_QUIET`) | `guards.release` |
| 15 | Work budget: the floor reads at most 6× the line's length + 64 KiB across all nesting levels, or the line is refused as unreadable | `crates/purlis-core/src/floorguard.rs:229` (`WORK_FACTOR`), `:233` (`WORK_SLACK`) | `guards.release` |
| 16 | Depth limit: commands inside commands are read up to 16 deep; past that the line is refused | `crates/purlis-core/src/floorguard.rs:268` (`MAX_DEPTH`); `crates/purlis-core/src/guardcaps.rs:63` | `guards.release` |
| 17 | The merge hold takes precedence: in an app-started chat, a line the hold reads as a merge goes to the hold, not to A4 (the hold reuses the floor's merge half, `merge_reason`) | `crates/purlis-core/src/toolgate.rs:381`; `crates/purlis-core/src/floorguard.rs:170` (`merge_reason`), `:196` (`MERGE_ROWS`) | `guards.merge` |

### handoff guard

| # | Decision | Where | Family |
|---|---|---|---|
| 1 | A helper sub-agent may not run `purlis handoff`; "sub-agent" means a non-empty `agent_id` on a harness where it was measured | `crates/purlis-core/src/handoffguard.rs:647` (`from_a_subagent`), `:88` (`MEASURED_SUBAGENT_HARNESSES` claude-code, codex) | `guards.shape` |
| 2 | A handoff inside a string or heredoc that a shell runs (`eval`, `sh -c`, fed to a shell, `find -exec`) is refused | `crates/purlis-core/src/handoffguard.rs:348` (`shell_string_handoff`), `:157` (`REASON_SHELL_STRING`) | `guards.shape` |
| 3 | A handoff whose words are reached only through an expansion, glob or substitution (`SPELLING_MARKS`), or that is not at the start of its own command, is refused | `crates/purlis-core/src/handoffguard.rs:385` (`handoff_segment`), `:93` (`SPELLING_MARKS`) | `guards.shape` |
| 4 | The brief must come from exactly one quoted heredoc in the same call. Refused: here-string, `<` file, pipe, no heredoc, more than one heredoc, xargs/parallel, `<&`, an unquoted heredoc, a live substitution anywhere in the call | `crates/purlis-core/src/handoffguard.rs:1018` (`brief_source`); `:100` (`REDIRECT_READS`), `:970` (`STDIN_TO_WORDS`) | `guards.shape` |
| 5 | A report-back handoff (`--report`) needs no heredoc; only a live substitution in it is refused | `crates/purlis-core/src/handoffguard.rs:783` | `guards.shape` |
| 6 | `purlis handoff --help`/`-h` with nothing fed in, and only one handoff on the line, passes | `crates/purlis-core/src/handoffguard.rs:823` (`asks_for_help`) | `guards.shape` |
| 7 | A handoff in a `case` branch, a function body, or nested deeper than the first level is refused; past the nesting cap it is `too-big-to-check` | `crates/purlis-core/src/handoffguard.rs:569` (`placed_in`), `:707` | `guards.shape` |
| 8 | An unattended (`bypassPermissions`) handoff is no longer refused here; the app decides it under the dispatch grant | `crates/purlis-core/src/handoffguard.rs:696` (doc); see `dispatchunattended` | `dispatch` |

## Asks and consents

| # | Decision | Where | Family |
|---|---|---|---|
| 1 | Only a human scope (the window, or `charter inbox`) answers an ask. A chat can raise an ask but never answer one. | `crates/purlis-core/src/harness/asks.rs:489` (`may_answer`), `:496` (`may_choose`), `:504`; hook side: `crates/purlis-core/src/harness/hooked.rs:19-21` doc, `crates/purlis-cli/src/permission.rs:35` | `consents` |
| 2 | Some asks are answered from the window only, not from `charter inbox`: an ask that elicits a secret, an Allow on an edit, and an answer to a chat's question. Deny and cancel can come from either. | `crates/purlis-core/src/harness/asks.rs:489`, `:496` | `fixed` |
| 3 | "Allow always" (an allow that outlives the session) is never offered and is refused if sent, whatever the source. | registry: `crates/purlis-core/src/harness/asks.rs:287-289` (`raise` partitions out `lasts`, `:482`); hook path: `crates/purlis-core/src/harness/hooked.rs:160` | `consents` |
| 4 | A permission option that switches the session's mode (Claude Code `setMode`, e.g. bypassPermissions or acceptEdits) is never offered in the window. It is left to the pane. | `crates/purlis-core/src/harness/hooked.rs:183` (`switches_the_mode`), applied at `:160` | `mode` |
| 5 | Allow is dropped when the window's one-line summary is not the whole action (cut, masked, hidden Unicode), or when a shell call has input keys beyond `command`/`description`/`timeout` (e.g. `dangerouslyDisableSandbox`, `run_in_background`). Only deny is left; the pane shows the rest. | `crates/purlis-core/src/harness/hooked.rs:174`; `PLAIN_SHELL_KEYS` `:198`; `shown_in_full` `:214` | `consents` |
| 6 | An edit may be allowed from the window only "this once", only with its whole change shown. Rule-, directory- and session-scoped allows on an edit are dropped. | `crates/purlis-core/src/harness/hooked.rs:171` | `consents` |
| 7 | An opencode command or edit is never allowed from the window, because its ask names opencode's matched patterns, not the line that runs. | `crates/purlis-core/src/harness/hooked.rs:173-176` (`opencode_line`) | `consents` |
| 8 | Options whose label hides characters, or carries a vault value this process knows, are not offered. | `crates/purlis-core/src/harness/hooked.rs:162-166` | `secrets` |
| 9 | Each harness's permission prompt is raised as an ask in the window. A `PermissionRequest` hook goes on every tool (no matcher), armed on the CLI argument and never in a plugin file a chat can write. | Claude: `crates/purlis-core/src/harness/claude.rs:1401` (`permission_hook`); Codex: `crates/purlis-core/src/harness/codex.rs:539` (`permission_flag`); opencode: shim `permission.asked` → `crates/purlis-core/src/harness/hooked.rs` `Source::Opencode`; ACP: `crates/purlis-core/src/harness/asked.rs:262` | `mode` |
| 10 | Permission hook timeout: 60 s. purlis writes this value into the harness's hook config. | `crates/purlis-core/src/harness/hooked.rs:41` (`HOOK_TIMEOUT`), written at `crates/purlis-core/src/harness/claude.rs:1411` and `crates/purlis-core/src/harness/codex.rs:539ff` | `limits` |
| 11 | A hook ask's deadline is 2 s before the hook timeout (about 58 s). The CLI hook waits at most the timeout minus 1 s. | `crates/purlis-core/src/harness/model.rs:305` (`HOOK_MARGIN`); `crates/purlis-cli/src/permission.rs:32` (`WAITS_AT_MOST`) | `limits` |
| 12 | On timeout, or with no window or no listener, purlis neither allows nor denies. The hook prints nothing and the harness's own pane prompt decides. | `crates/purlis-core/src/harness/hooked.rs:14-17`; `crates/purlis-cli/src/permission.rs:19-21` (the `permissionrequest` flow, `:35`) | `mode` |
| 13 | A timed-out hook ask stays answerable as "lapsed" for 30 min. A late answer is kept as a pre-answer, also for 30 min and spent once, and it auto-answers the chat's next identical request. | `crates/purlis-core/src/harness/hooked/lapsed.rs:31` (`LAPSED_FOR`), `:34` (`PRE_ANSWERED_FOR`), `:37` (`MOST_LAPSED`) | `consents` |
| 14 | ACP permission asks have no deadline: nothing answers them on a timer. | `crates/purlis-core/src/acp.rs:34-36` doc; `crates/purlis-core/src/harness/asked.rs:258-262` | `limits` |
| 15 | At most 32 open asks per chat. One more is not raised, and the pane decides it (hook) or it is refused (ACP). | `crates/purlis-core/src/acp.rs:349` (`MOST_OPEN_ASKS`); reused at `crates/purlis-core/src/harness/hooked.rs:45`, checked `:578`, `crates/purlis-core/src/acp.rs:1443` | `limits` |
| 16 | An ask payload over 64 KiB is not raised, and is not cut short. | `crates/purlis-core/src/acp.rs:393` (`MOST_ASK_BYTES`), `crates/purlis-core/src/harness/hooked.rs:49` | `limits` |
| 17 | The window remembers 4096 closed asks so that a late answer hears why it was refused. | `crates/purlis-core/src/harness/asks.rs:203` (`REMEMBERED`) | `limits` |
| 18 | A task's question (`AskUserQuestion`; Codex `request_user_input`; opencode `question`) goes to the chat that dispatched it, not to the person. Permission prompts are never routed this way. | `crates/purlis-cli/src/permission.rs:102` (`redirected`), `:128` (`taskquestion`); Codex hook `crates/purlis-core/src/harness/hooked.rs:309-317`; `crates/purlis-core/src/harness/hooked.rs:asked_text` | `dispatch` |
| 19 | Risk class: a push to a protected branch. A protected branch is a repo's default branch in `inventory/repos.json`, plus `[risk] protected_branches` in `charter.toml` (globs). `--all`/`--mirror`/`--branches` counts when any protected branch exists. A bare push is judged by its upstream branch. | `crates/purlis-core/src/askrisk.rs:109` (`Facts::read`), `:49`, `:315` | `asks` |
| 20 | Risk class: a production host. A host counts if it has a label `prod`/`production`, or is named (with its subdomains) in `[risk] production_hosts`. | `crates/purlis-core/src/askrisk.rs:179-188`, `:51` | `asks` |
| 21 | Risk class: a write outside the project root (lexical, `~` expanded). | `crates/purlis-core/src/askrisk.rs:203` | `asks` |
| 22 | Risk class: a delete (`rm`, `rmdir`, `unlink`, `git rm` without `--cached`, `find -delete`, `git branch -d/-D`, `git push --delete`/`:ref`, ACP `delete` kind), a vault use, a merge. Shell nesting is read 4 levels deep. | `crates/purlis-core/src/askrisk.rs:470` (`deletes`), `:388`, `:394`, `:406`, `:54` (`SCRIPT_DEPTH`) | `asks` |
| 23 | A risky ask is never grouped and never offered a rule, and its Allow asks a second confirm. | `app/src-tauri/src/asking.rs:1309` (`high_risk`), `:1352` (`offered`); confirm text `crates/purlis-core/src/askrisk.rs` `Risk::confirm` | `asks` |
| 24 | Offer a rule after the same Allow to the same request 2 times within 7 days, with no other answer between. The suggested scope is the narrowest that would have prevented every ask (chat, then project, then machine). | `crates/purlis-core/src/askrules.rs:37` (`OFFER_WINDOW_SECS`), `:40` (`OFFER_AFTER`), `:177` (`offer`); scope pick `app/src-tauri/src/asking.rs:1352` | `asks` |
| 25 | Cool-off: 2 "Don't allow" from one chat to one request within 1 h hold that chat's same ask quietly (not counted, not notified) for 1 h after the last one. | `crates/purlis-core/src/askrules.rs:43-49` (`COOL_AFTER`, `COOL_WITHIN_SECS`, `COOL_FOR_SECS`), `:222` (`cooling`); applied `app/src-tauri/src/asking.rs:1392` (`with_rules`) | `asks` |
| 26 | Which asks can become a rule (named requests): sandbox host, write or local; a hook permission whose line is one line and is not an edit; a run grant for a program with no fills. | `app/src-tauri/src/asking.rs:1318` (`request_of`) | `asks` |
| 27 | A merge answer is never remembered as a rule ("no grants for merges"). | `crates/purlis-core/src/askrules.rs:337` (`Store::make`); `crates/purlis-core/src/mergehold.rs:16` | `guards.merge` |
| 28 | A harness's own saved "from now on" rule is never offered as an Asks rule. | ADR 0121 amendment 2026-11-11; enforced by #3 and `crates/purlis-core/src/layer.rs:155` | `asks` |
| 29 | Remembered-answer store `<data>/inbox/rules.json`: at most 2000 answers, 200 rules, 4 MiB. An unreadable file reads as empty. | `crates/purlis-core/src/askrules.rs:31`, `:56`, `:59`, `:62-69` | `limits` |
| 30 | Scopes an Allow may be kept at (once, chat, you, machine, project). Policy can remove any of them. | `crates/purlis-core/src/sandbox/policy.rs:852` (`allow-scopes`), `:625` (`keeps_scope`), `:633` (`scope_removed`); applied per ask in `app/src-tauri/src/asking.rs:1558`, `:1640`, `:1752`, `:1801`, `:1901`, `:3459`, `:3548` | `asks` |
| 31 | Scopes offered per ask kind. Sandbox host: you/chat/project + Keep blocked. Folder: chat/you (a protected folder: chat only). Port: chat/you. Socket/service: once/machine/chat/you. Runner service: once/chat. Browser sites: chat/you/project. Run grant: once / chat (reviewed program only) / always (persona + reviewed program + project scope kept). | `app/src-tauri/src/asking.rs:1558`, `:1640-1642`, `:1801`, `:1901`, `:1752`, `core`→`app/src-tauri/src/asking.rs:3548` (`browser_site_answers`), `:3459` (`program_options`), `:3285-3300` | `asks` |
| 32 | Dispatch grant ask: Allow at the levels offered (chat / you / project), Keep blocked, and "Never for this pair" (only when the asker runs as a persona). A policy lock offers nothing. | `app/src-tauri/src/asking.rs:1458` (`dispatch`); `crates/purlis-core/src/dispatchgrant.rs:16-27` | `dispatch` |
| 33 | A live (held) connection waits 60 s for the person, then times out and becomes a block. | `crates/purlis-core/src/sandbox/asks.rs:55` (`HOLD`); window `app/src-tauri/src/asking.rs:4703` (`hold_ends`), `:4698` (`LOOK_AGAIN` = 5 s) | `asks` |
| 34 | While Quiet, a held connection past its hold waits on, up to 15 min in all. New hosts within 2 s are grouped as one ask. At most 16 held at once; more are refused unasked. | `crates/purlis-core/src/sandbox/asks.rs:59` (`MOST_HELD_OPEN`), `:62` (`GROUP`), `:65` (`MOST_HELD`) | `asks` |
| 35 | "Allow next time" (a one-shot grant with no ask open) lives in memory only and is lost on a restart. | `crates/purlis-core/src/sandbox/asks.rs:26-33` doc (`Asks::allow_next`) | `asks` |
| 36 | Secret ask: a brokered command waits ¾ of the harness's default command timeout (Claude/opencode 120 s → 90 s; Codex 10 s → 7.5 s; unknown harness 10 s). After that the ask stays open, and the chat is told to retry. | `crates/purlis-core/src/secrets/ask.rs:529` (`holds_for`), `:516` (`default_command_timeout`), `:511`; extra waits `:1996`, `:2004` | `vaults` |
| 37 | Secret ask: one ask per vault/key per project. A decline holds for that chat for the rest of its life (memory only, and a restart forgets it). Open asks persist across an app restart in `<state>/app/secret-asks.json`. | `crates/purlis-core/src/secrets/ask.rs:387` (declined set), `:884`, `:76` (`KEPT_IN`) | `vaults` |
| 38 | Secret ask for a key the persona does not declare: refused as `declined: policy`, and the person is never asked. | `crates/purlis-core/src/secrets/ask.rs:617`, `:890`, `:270` | `vaults` |
| 39 | Secret ask may offer "remember on the persona" only where the persona's definition is offerable. | `app/src-tauri/src/asking.rs:3081` (`may_remember` param); `app/src-tauri/src/proposals.rs:1307` (`need_offerable`) | `vaults` |
| 40 | Run grant (a program the persona does not declare, run with a vault): offered unless locked. "Always allow" writes a tool line into the persona's committed file. | `app/src-tauri/src/asking.rs:3253` (`run_grant`), `:3459`; `crates/purlis-core/src/settings/always_allow.rs` | `vaults` |
| 41 | Browser login a chat suggests: https only, no IP address or single-label host. The page origin and the form origin are both confirmed. A key named by a `mint` line is never offered. | `crates/purlis-core/src/loginask.rs:213` (`site_of`), `:47` (`MOST_SITE`=512) | `browser` |
| 42 | Browser login / secret fill / pool tool wait 15 min for the person, then the notice goes and the chat may ask again. | `crates/purlis-core/src/loginask.rs:43` (`THE_PERSON_IS_WAITED_FOR`); used for fills `app/src-tauri/src/asking.rs:4880-4900` | `browser` |
| 43 | Browser login: one proposal waits per chat (another call is refused). At most 2 grants are kept. "Once" and "this chat" grants are memory only. "Always" writes the persona line, and only if the persona list was already seen. | `app/src-tauri/src/loginasks.rs:133` (`ALREADY_ASKED`), `:139` (`KEPT_AT_MOST`) | `browser` |
| 44 | Requests handed on from a runner while the person is away: at most 16 kept, 4 per chat, each for at most 15 min. A waited login is answered within 60 s once the person is back. | `crates/purlis-core/src/hookwire/relay.rs:280`, `:283`, `:288`, `:297` | `runners` |
| 45 | Workspace removal asked by a chat: only the person's press in the Delete dialog removes it. At most 3 questions per chat. Memory only. | `crates/purlis-core/src/removalask.rs:26` (`named`), `:81` (`holds`); `app/src-tauri/src/removalasks.rs:45` (`AT_MOST_PER_CHAT`) | `guards.plane` |
| 46 | Merge ask: a chat never merges. The app merges on the person's press, only while the request's head is the commit asked about, and only for repos in the inventory. | `crates/purlis-core/src/mergehold.rs:4-27`; `app/src-tauri/src/asking.rs:2158` (`merge_hold`) | `guards.merge` |
| 47 | A window-only refusal (e.g. Trust this clone) becomes an ask, one per chat per control. Unrecorded clones are trusted only through one project-wide ask. | `crates/purlis-core/src/windowonly.rs:4-25`, `:69`, `:240`; `app/src-tauri/src/asking.rs:2234`, `:2289` | `consents` |
| 48 | The project's "expects" consent (Allow on this machine, ADR 0093) and a persona's hosts/tools waiting for an Allow are Blocking asks. A changed list asks again. | `app/src-tauri/src/asking/consents.rs:222` (`persona_allows`); ADR 0121 amendments 2026-11-10 and 2026-11-12 | `trust` |
| 49 | A `purlis` CLI command's confirmation in the window (pool sign-in, profile off a pool, signing check) waits 2 min, and a timeout counts as no. With no project open in the window it is refused at once. | `crates/purlis-core/src/windowline.rs:52` (`THE_PERSON_HAS`) | `fixed` |
| 50 | Report consent: two streams (Crash reports, Agent drafts) plus "Always send". Unanswered, unreadable or missing means no. Asked once at first run, and once more after a crash. Window only; a committed file can never give it. Stored as `<config>/reports.json`, Machine tier. | `crates/purlis-core/src/report/consent.rs:51` (`Consent`), `:94` (`allows`), `:104` (`always_sends`), `:111` (`wants_the_crash_notice`), `:117` (`read`); `app/src-tauri/src/reportconsent.rs:8` (WINDOW_ONLY) | `consents` |
| 51 | Consent-gated commands (`report … --yes`, `todo … promote`): `init` writes `ask` rules for them into `.claude/settings.json` and `opencode.json`, in both the `charter` and `purlis` spellings. The retired handoff `ask` rule is removed. | `crates/purlis-core/src/scaffold/settings.rs:51-95` (`REPORT_RULE`, `PROMOTE_RULE`, `PURLIS_*`), `:134` (`handoff_rule_retired`) | `consents` |
| 52 | A consent-gated command written in a spelling the host's ask rule does not match (a path, another case, `VAR=`, a wrapper, inside `sh -c` or a heredoc) is refused, fail-closed. | `crates/purlis-core/src/consentspelling.rs:73` (`REASON`), `:81` (`GUARDED`) | `guards.shape` |
| 53 | The person's own `ask`/`deny` Bash rules are enforced under every spelling of the named program (backstop for host globs). `allow` rules are not. | `crates/purlis-core/src/rulespelling.rs:52` (`REASON`), `:316` (`MAX_STARTS_TRIED`) | `fixed` |
| 54 | `purlis guard ask/allow`: writes rules into each harness's own file, all or nothing across harnesses. `allow` reaches the plane root only. Workspace and clone layers carry `ask`/`deny`, never `allow`. | `crates/purlis-core/src/guardcmd.rs:161`; `crates/purlis-core/src/layer.rs:155` (`RESTRICTIVE`); per harness: `crates/purlis-core/src/harness/claude.rs:373`, `crates/purlis-core/src/harness/opencode.rs:198`, `crates/purlis-core/src/harness/codex.rs:370` (none) | `tools` |
| 55 | Attendance: a chat is unattended if it was begun as a task, a runner or a loop, if its profile command switches prompts off, or (Claude only) if its armed hook reports `bypassPermissions`. It only ever moves toward unattended, and carries over across resume and relaunch. | `crates/purlis-core/src/attendance.rs:75` (`Begun::unattended`), `:245` (`at_start_reading`), `:264` (`resumed`), `:181` (`of_a_line`), `:158` (`PROMPTS_OFF`) | `mode` |
| 56 | Attendance fails closed: a chat the app holds no fact for, or an app that does not answer, counts as unattended. | `crates/purlis-core/src/attendance.rs:199` (`of_held`); `crates/purlis-core/src/toolgate.rs:180` | `mode` |
| 57 | A handoff from an unattended chat starts unattended. | `crates/purlis-core/src/attendance.rs:56-60` (Begun::Person doc), via `Fact::resumed` | `mode` |
| 58 | Release floor (A4): an unattended chat may not publish (release/tag/forge publish). It is denied, not asked. | `crates/purlis-core/src/floorguard.rs:161` (`release_floor_reason`); `crates/purlis-core/src/toolgate.rs:369` | `guards.release` |
| 59 | Commit-gate nudge is never shown to an unattended chat. | `crates/purlis-core/src/commitgate.rs:215` | `guards.save` |
| 60 | An unattended chat dispatches only under a standing grant for "you" or "project" (never a chat-level grant). A missing grant is an immediate refusal, never a Notice. An unattended chat with no sandbox dispatches to no other persona. | `crates/purlis-core/src/dispatchunattended.rs:185` (`covers`) | `dispatch` |
| 61 | An unattended chat's handoff may not `--create` a workspace. | `crates/purlis-core/src/dispatchunattended.rs:309` (`NO_WORKSPACE_IS_MADE`); `app/src-tauri/src/handoff.rs:2825` | `dispatch` |
| 62 | Silence notes: a task with no harness activity for 15 min, or one command running 30 min, is noted to the asker and on the task's tab. Nothing is ended. | `crates/purlis-core/src/dispatchlimits.rs:273-274`, keys `:184-185`; rules `crates/purlis-core/src/silence.rs`; app `app/src-tauri/src/silence.rs:13` | `limits` |
| 63 | Asks are notified only when the person is not looking and not Quiet: quiet hours, "Quiet now", and 30 s after typing in a chat. | `app/src-tauri/src/asknotify.rs:1228` (`TYPING_QUIET`); quiet hours in `layout.json` | `consents` |
| 64 | Notification batching: chats asking within 5 s of each other are one burst, and 3 or more are told together. | `app/src-tauri/src/asknotify.rs:105` (`TOGETHER`), `:392` (`MANY`) | `consents` |
| 65 | Expiring-ask sound fires 15 s before an ask expires. | `app/src-tauri/src/asknotify.rs:272` (`SOUND_BEFORE`), `:313-318` | `consents` |
| 66 | "Should know" updates: a task that finishes after more than 5 min while the person is away is notified. | `app/src-tauri/src/asknotify.rs:266` (`FINISHED_AFTER`), `:1357-1362` | `consents` |
| 67 | What a chat is told of the answer: a fixed purlis line. The person's reason is capped at 200 chars and JSON-quoted. | `crates/purlis-core/src/askoutcome.rs:27` (`MOST_REASON`), `:293` (`Outcome::line`) | `asks` |

## Dispatch


### Who may dispatch at all, and in what order it is judged

| # | Decision | Where | Family |
|---|---|---|---|
| 1 | Order of checks for every dispatch/handoff: helper → held → persona exists/not draft → policy → profile → limits → grant → memory | `crates/purlis-core/src/dispatchdecision.rs:608` (`decide`) | `dispatch` |
| 2 | A harness's helper sub-agent may not dispatch (Bash `purlis dispatch`, any `dispatch_*` MCP tool) | `crates/purlis-core/src/dispatchguard.rs:190` (`refusal`), `:236` (`tool_refusal`); decision `crates/purlis-core/src/dispatchdecision.rs:613`; sub-agent detection `crates/purlis-core/src/handoffguard.rs:647` | `guards.shape` |
| 3 | A helper sub-agent may not run `purlis worktree add/commit/push` (task work) | `crates/purlis-core/src/dispatchguard.rs:106` (`A_TASKS_WORK`), `:190` | `guards.shape` |
| 4 | Claude Code `Task`/`Agent` with `subagent_type` naming a persona is denied (persona is never a sub-agent) | `crates/purlis-core/src/toolhooks.rs:706`; sentence `crates/purlis-core/src/personaverbs/retired.rs:53` | `dispatch` |
| 5 | A chat still holding another persona's grants (handoff/Resume hold) cannot dispatch; the person from its tab can | `crates/purlis-core/src/dispatchdecision.rs:620`; `app/src-tauri/src/dispatchgrants.rs:273,624` | `dispatch` |
| 6 | Target must be a defined, loadable, non-draft persona; a dispatch from a chat whose persona no longer exists is refused | `crates/purlis-core/src/dispatchdecision.rs:624-626`, `:690` (`persona_in`); `crates/purlis-core/src/dispatchdormant.rs:188` | `dispatch` |
| 7 | Chat on a persona must name a persona (`--to`), else its own | `crates/purlis-core/src/dispatchdecision.rs:309` (`pair_of`), `:661` | `dispatch` |
| 8 | The person dispatching from a chat's tab needs no grant and is not held to nevers or to "held" | `crates/purlis-core/src/dispatchdecision.rs:656` | `dispatch` |
| 9 | Policy `dispatch.allow: false` forbids all dispatch, own persona included | `crates/purlis-core/src/sandbox/policy.rs:1063`; `crates/purlis-core/src/dispatchgrant.rs:448` | `dispatch` |
| 10 | Policy `dispatch.locked` pairs: no grant covers them, Notice offers no Allow | `crates/purlis-core/src/sandbox/policy.rs:1068`, `:812`; `crates/purlis-core/src/dispatchgrant.rs:457` | `dispatch` |
| 11 | A refused (unreadable) policy file switches dispatch off | `crates/purlis-core/src/sandbox/policy.rs:368-369`; `crates/purlis-core/src/dispatchlimits.rs:1385`; `crates/purlis-core/src/dispatchdecision.rs:634` | `dispatch` |
| 12 | Task name must be a valid chat label without `⟨ ⟩ · \``; empty refused | `crates/purlis-core/src/dispatchdecision.rs:279` | `dispatch` |
| 13 | A task with the exact same name still in flight from that chat → refused | `crates/purlis-core/src/dispatchdecision.rs:968` (`same_name_open`) | `dispatch` |
| 14 | After a restart, same brief to same persona while the earlier task still works → refused | `crates/purlis-core/src/dispatchrestart.rs:627` (`dispatched_before`); `app/src-tauri/src/restored.rs:416` (`refuse_twice`), called `app/src-tauri/src/handoff.rs:2814` | `dispatch` |
| 15 | Brief shaped like a credential is refused (dispatch and handoff) | `crates/purlis-cli/src/dispatch.rs:375`; `crates/purlis-cli/src/handoff.rs:241` (`secretshape::kind_as_read`) | `secrets` |
| 16 | Memory short: a chat's dispatch waits (not refused); person's own does not | `crates/purlis-core/src/dispatchdecision.rs:235` (`waits`) | `limits` |
| 17 | Memory wait gives up after 10 minutes, starts nothing | `crates/purlis-core/src/dispatchdecision.rs:242,246` (`MEMORY_WAIT`) | `limits` |

### Grants (who may dispatch to whom)

| # | Decision | Where | Family |
|---|---|---|---|
| 18 | Same persona needs no grant; any other pair needs one (else person asked via Notice) | `crates/purlis-core/src/dispatchgrant.rs:447` (`covers`) | `dispatch` |
| 19 | Grant levels: this chat (memory), me on this machine (`app/src-tauri/src/sandbox.json` `dispatch_mine`), everyone in project (manifest `[dispatch.grants]`) | `crates/purlis-core/src/dispatchgrant.rs:228` (`InForce::read`), `:123` (`KEY`); `crates/purlis-core/src/sandbox/local.rs:1993` (`grant_dispatch`) | `dispatch` |
| 20 | "Any persona" grant (`*`) — only from Settings, never from a Notice | `crates/purlis-core/src/dispatchgrant.rs:128` (`ANY`), `:725` (`allow_any`); `crates/purlis-core/src/sandbox/local.rs:2102`; `app/src-tauri/src/dispatchgrants.rs:2862` | `dispatch` |
| 21 | A committed project grant counts on this machine only after a person here accepts it | `crates/purlis-core/src/dispatchgrant.rs:855` (`acknowledge_pair`); `crates/purlis-core/src/dispatcharrival.rs:871` (`arrival`) | `dispatch` |
| 22 | Acceptance dropped when a commit removes the grant, or history unreadable (>2000 commits, object >16 MiB, file >1 MiB, shallow/grafts, git timeout) | `crates/purlis-core/src/dispatcharrival.rs:629` (`settle`), `:78,81,84,88` | `dispatch` |
| 23 | "Not on my machine" declines one project grant locally | `crates/purlis-core/src/dispatchgrant.rs:794` (`decline`); `app/src-tauri/src/dispatchgrants.rs:2559,2919` | `dispatch` |
| 24 | Grant condition: any workspace, or only tasks that *work in* one named workspace; task at project root matches no limited grant | `crates/purlis-core/src/dispatchwithin.rs:121` (`works_in`), `:232-235`; `app/src-tauri/src/handoff.rs:2832` | `dispatch` |
| 25 | Never for a pair (`app/src-tauri/src/dispatch-never.json`) beats every grant | `crates/purlis-core/src/dispatchnever.rs:264` (`read`), `:294` (`add`); `crates/purlis-core/src/dispatchgrant.rs:463` | `dispatch` |
| 26 | Never holds down the chain (persona above in lineage) | `crates/purlis-core/src/dispatchgrant.rs:423` (`covers_in_chain`) | `dispatch` |
| 27 | Unreadable never-file: no grant counts; attended → asked, unattended → refused | `crates/purlis-core/src/dispatchnever.rs:264`; `crates/purlis-core/src/dispatchgrant.rs:471`; `crates/purlis-core/src/dispatchunattended.rs:225` | `dispatch` |
| 28 | "Keep blocked" — same chat asking same pair again refused at once for the chat's life | `app/src-tauri/src/dispatchgrants.rs:1527` | `dispatch` |
| 29 | At most 5 dispatches waiting on the person per chat | `app/src-tauri/src/dispatchgrants.rs:211,875` | `limits` |
| 30 | Grant Notice shows brief inert, cut at 12 288 bytes; task name cut at 120 chars | `crates/purlis-core/src/dispatchgrant.rs:996`; `app/src-tauri/src/dispatchgrants.rs:316` | `limits` |
| 31 | Grant in force only while both personas exist; a returning name with a different definition has grants set aside until Give back | `crates/purlis-core/src/dispatchdormant.rs:188` (`judged`), `:230` (`persona_gone`), `:281` (`give_back`) | `dispatch` |
| 32 | A chat can never create/widen/revoke a grant: brokered write and chat commit refuse any `[dispatch]` change | `crates/purlis-core/src/brokered.rs:1287` (`RUNS_UNDER_BESIDES_SANDBOX`), `:1306` (`guard`), `:1381`; `crates/purlis-core/src/diffscan.rs:91` | `fixed` |
| 33 | `wants:` in persona.md only offers ticked-off boxes; ≤6 offered, ≤40-char names | `crates/purlis-core/src/dispatchwants.rs:67,77`, `:284` (`also`) | `dispatch` |
| 34 | A covered dispatched chat starts with the target's own grants, none of the asker's, no opt-out | `crates/purlis-core/src/dispatchgrant.rs:985`; `crates/purlis-core/src/dispatchunattended.rs:375` | `dispatch` |

### Unattended chats (prompts off / tasks)

| # | Decision | Where | Family |
|---|---|---|---|
| 35 | Unattended chat: only standing grants (you/project) count; chat-level grants ignored; missing grant = refusal, never a Notice | `crates/purlis-core/src/dispatchunattended.rs:194` (`covers`), `:212` (`answer_of`); `app/src-tauri/src/dispatchunattended.rs:137` | `dispatch` |
| 36 | Unattended **and unsandboxed** chat dispatches to no persona but its own (`Refusal::Unsandboxed`) | `crates/purlis-core/src/dispatchunattended.rs:227`; `app/src-tauri/src/handoff.rs:2635` (`crossing_stands_aside`) | `dispatch` |
| 37 | Attendance = app's fact from how purlis started the chat, lowered by armed hook; no fact ⇒ unattended | `crates/purlis-core/src/dispatchunattended.rs:10-14` (doc), `crates/purlis-core/src/attendance.rs` (`of_held`); `app/src-tauri/src/handoff.rs:2556,2816` | `mode` |
| 38 | Unattended chat may not cross into another workspace except under a standing grant naming the pair and that workspace (not "any persona", not own persona) | `crates/purlis-core/src/dispatchplace.rs:856` (`nobody_to_ask`); `app/src-tauri/src/handoff.rs:2882` | `dispatch` |
| 39 | Unattended chat's handoff may not `--create` a workspace | `crates/purlis-core/src/dispatchunattended.rs:309`; `app/src-tauri/src/handoff.rs:2825` | `dispatch` |
| 40 | Refusal for lack of grant kept for the person (needs-you list), ≤50 entries, 30 days, dismiss quiet 7 days | `crates/purlis-core/src/dispatchaway.rs:92,95,99`, `:389` (`keep`); `app/src-tauri/src/dispatchaway.rs` | `limits` |
| 41 | No dispatched chat starts on a profile whose command switches prompts off (`--dangerously-skip-permissions`, `--yolo`, `--full-auto`, `-a never`, `-c approval_policy=never`, `--permission-mode bypassPermissions`), whoever named it | `crates/purlis-core/src/dispatchunattended.rs:241,252` (lists), `:278` (`bypass_in`), `:331` (`bypass_refusal`) | `mode` |

### Mode the task runs in

| # | Decision | Where | Family |
|---|---|---|---|
| 42 | Task mode = asker's mode capped at auto: default→default, auto→auto, bypass→auto; dontAsk/acceptEdits/plan→default | `crates/purlis-core/src/dispatchmode.rs:93` (`for_a_task`) | `mode` |
| 43 | Asker's mode believed only from its armed hook; otherwise reads "asks" | `crates/purlis-core/src/dispatchmode.rs:104` (`of_its_own_harness`) | `mode` |
| 44 | Profile naming its own `--permission-mode` wins; nothing added | `crates/purlis-core/src/harness/claude.rs:449` (`task_mode_words`) | `mode` |
| 45 | Auto refused for a task where person's/managed Claude Code settings carry auto deny rules → starts in default | `crates/purlis-core/src/dispatchmode.rs:140` (`task_start`); `crates/purlis-core/src/harness/claude.rs:469` | `mode` |
| 46 | Grade's chat gets no mode words | `crates/purlis-core/src/dispatchmode.rs:147` | `fixed` |

### Limits (`[dispatch]` table)

| # | Decision | Where | Family |
|---|---|---|---|
| 47 | `running-per-chat`: tasks one chat may have running | `crates/purlis-core/src/dispatchlimits.rs:269`, `:930` | `limits` |
| 48 | `live-per-lineage`: live chats under one person-started chat | `:270`, `:932` | `limits` |
| 49 | `depth`: dispatch chain depth | `:271`, `:920` | `limits` |
| 50 | `depth` never above 8, at any level incl. policy | `:114` (`DEEPEST`), `:318` | `limits` |
| 51 | `messages-per-minute` per pair of chats (both ways); 0 stops messages only | `:272`, `:1051` (`may_send`); `crates/purlis-core/src/dispatchtalk.rs:202` | `limits` |
| 52 | `may-dispatch`: running tasks across all chats of the asking persona (persona table only) | `:283`, `:939` | `limits` |
| 53 | `may-run-at-once`: chats running as target persona at once | `:283`, `:948` | `limits` |
| 54 | `tokens-per-session`: session token budget; new task refused, tasks paused (Continue/Raise/Stop) | `:283`, `:1030` (`tokens_refused`) | `limits` |
| 55 | `minutes-per-task`: working-time budget; task paused | `:283`, `:1014` (`time_reached`) | `limits` |
| 56 | `quiet-note-minutes` / `one-command-note-minutes`: when purlis notes a quiet task/long command (never ends it) | `:275-276` | `limits` |
| 57 | `stuck-silent-minutes` / `stuck-failing-times` / `stuck-long-minutes`: when a chat reads "Stuck?" | `:277-279` | `limits` |
| 58 | Any limit read at most 10 000; tokens at most 1 000 000 000 | `:117,120` | `limits` |
| 59 | 0 switches dispatch off at that level (more specific may re-enable); policy 0 cannot be lifted | `:301-310` (`switches_off`), `:904` | `limits` |
| 60 | Local (`charter.local.toml`) value above the project's is ignored | `:443`, `:1432` (`Files::of`) | `limits` |
| 61 | Loop rule: never dispatch to a persona already above the asker in its chain (own persona allowed) | `:909` | `dispatch` |
| 62 | Chain unreadable ⇒ any other persona refused as if a loop; recovered from dispatch records (≤2000 read) | `:916`; `crates/purlis-core/src/dispatchchain.rs:60,80` | `dispatch` |
| 63 | Policy ceiling on each limit | `crates/purlis-core/src/dispatchlimits.rs:1349` (`ceiling`); `crates/purlis-core/src/sandbox/policy.rs:506` | `limits` |

### Place (where the task works / runs)

| # | Decision | Where | Family |
|---|---|---|---|
| 64 | `--in` accepts only `worktree`, `worktree:<repo>`, `workspace:<name>`; no folder/branch can be named | `crates/purlis-core/src/dispatchplace.rs:287` (`asked`) | `dispatch` |
| 65 | Persona default `dispatch-isolation: worktree`; falls back to asker's folder if no repo | `crates/purlis-core/src/dispatchplace.rs:313` (`isolates`), `:611` (`ground`) | `dispatch` |
| 66 | Worktree folder/branch name is purlis's (task name + id); an existing one is a refusal | `crates/purlis-core/src/dispatchplace.rs:549` (`piece_name`), `:649` (`cut`) | `dispatch` |
| 67 | Only the person merges a task branch (fast-forward of the commit shown, or nothing) | `crates/purlis-core/src/dispatchplace.rs:763` (`MERGED_BY`), `:2143` (`merge`) | `guards.merge` |
| 68 | Discard of a task worktree: person only, refused while a chat stands in it, folder >100 000 entries refused | `crates/purlis-core/src/dispatchplace.rs:1415`; `app/src-tauri/src/dispatches.rs:33-38,1233` | `consents` |
| 69 | Merged task worktree removed by purlis only by git's safe removal, at chat close/project open | `crates/purlis-core/src/dispatchplace.rs:2198` (`tidy`) | `guards.git` |
| 70 | Sandboxed task commits/updates/pushes only via brokered `purlis worktree …` | `crates/purlis-core/src/dispatchplace.rs:752-759`; `crates/purlis-core/src/harness/claude.rs:1560` | `sandbox` |
| 71 | Run place order: start's choice → workspace → project → this machine | `crates/purlis-core/src/machine.rs:1912` (`chat_place`) | `dispatch` |
| 72 | A dispatch naming a pool this machine doesn't keep is refused | `app/src-tauri/src/handoff.rs:3719` (`names_a_pool_kept`), `:300` | `runners` |
| 73 | A task at project root cannot run on a pool | `app/src-tauri/src/handoff.rs:318-326` | `runners` |
| 74 | Dispatch naming a place other than where levels put it ("moves work") is held for person's Approve, whatever grant | `app/src-tauri/src/handoff.rs:3745` (`moves_work`), `:328` | `dispatch` |
| 75 | Once the person refuses a chat's move to a place, later moves there are refused without asking (chat's life) | `app/src-tauri/src/runnerdispatches.rs:184,327` | `dispatch` |

### Runners / pools as origin or target

| # | Decision | Where | Family |
|---|---|---|---|
| 76 | Any dispatch/handoff from a chat on a pool's runner is held for the person's Approve, own persona included; no "always approve" | `app/src-tauri/src/runnerdispatches.rs:4-16`, `:297` (`hold`); `app/src-tauri/src/handoff.rs:247,305` | `runners` |
| 77 | ≤8 held dispatches per chat, ≤8 per pool | `app/src-tauri/src/runnerdispatches.rs:29,33` | `limits` |
| 78 | Workspace naming a pool not kept / runner not kept starts no chat | `crates/purlis-core/src/runners.rs:741` (`runner_for`) | `runners` |
| 79 | Draining runner starts no new chat | `crates/purlis-core/src/runners.rs:28-31` (doc; `draining`) | `runners` |
| 80 | Pool limits: profileSize 128 MiB (≤1024), profileFiles 8192 (≤65 536), codeSize 1024 MiB, codeSendTime 10 min, signInWait 15, vouchWait 5 (clusters), answerWait 30, cpus 2000m, memory 4096 MiB, processes 1024, tmp 1024 MiB, suspendAfter 1440 min | `crates/purlis-core/src/poollimits.rs:216` (`ALL`), rows `:218-436`; `:492` (`check`) | `limits` |

### Talk between asker and task; waits; restart

| # | Decision | Where | Family |
|---|---|---|---|
| 81 | Messages only along the lineage: down to own tasks, up to own asker | `crates/purlis-core/src/dispatchtalk.rs:131` (`down`), `:148` (`up`) | `dispatch` |
| 82 | Wait/cancel/close/tell only on tasks the chat dispatched itself | `crates/purlis-core/src/dispatched.rs:385` (`owned`) | `dispatch` |
| 83 | A chat may not cancel/close a task the person typed into; task started by person cannot message up | `crates/purlis-core/src/dispatched.rs:416-422`; `crates/purlis-core/src/dispatchtalk.rs:150,159` | `dispatch` |
| 84 | Message/report text ≤4096 bytes, no control chars; pass-on note ≤240 bytes | `crates/purlis-core/src/handoff.rs:568` (`MOST_REPORT_BYTES`); `crates/purlis-core/src/dispatchrecord.rs:1896`; `crates/purlis-core/src/dispatchtalk.rs:2129` | `limits` |
| 85 | Queue ≤256 messages per chat; a confined command cannot queue | `crates/purlis-core/src/dispatchqueue.rs:46` | `limits` |
| 86 | Task question goes to asker first; reaches person after 100 s of asker's turn, or at once if asker gone | `crates/purlis-core/src/dispatchtalk.rs:343`, `:1231` (`look`) | `dispatch` |
| 87 | Wait default 100 s, max 540 s | `crates/purlis-core/src/dispatched.rs:349,353,356`; `crates/purlis-core/src/chattools.rs:629` (tool text) | `limits` |
| 88 | Reported task's program ended after 120 s (heard harness) / 20 s (unheard); cancel/stop prompt heard within 60 s | `crates/purlis-core/src/dispatched.rs:959,964,968,1199,1204` | `limits` |
| 89 | At launch a task owing a report is resumed with fixed CARRY_ON line, never its brief; none without conversation/profile | `crates/purlis-core/src/dispatchrestart.rs:72`, `:243` (`at_launch`) | `dispatch` |
| 90 | Report for a closed asker is kept and handed back when the asker is reopened | `crates/purlis-core/src/dispatchrestart.rs:45-57` (`take_back`, `hand_to`) | `dispatch` |

### Profiles

| # | Decision | Where | Family |
|---|---|---|---|
| 91 | Profile chosen: dispatch `--profile` → persona's `profile:` → asker's | `crates/purlis-core/src/personaprofile.rs:452` (`for_dispatch`) | `dispatch` |
| 92 | `[dispatch.profiles]` list narrows profiles a persona's tasks may use (inherited via `extends:`); unreadable ⇒ nothing starts | `crates/purlis-core/src/dispatchprofiles.rs:61-64`; `crates/purlis-core/src/personaprofile.rs:461-475` | `dispatch` |
| 93 | Profile must be offered by the project and its command approved on this machine | `crates/purlis-core/src/personaprofile.rs:197-207`, `:514-528` | `trust` |
| 94 | Persona's own profile not offered here ⇒ falls back to asker's profile | `crates/purlis-core/src/personaprofile.rs:528` | `dispatch` |
| 95 | Persona with `disallowed-tools:` refused on a harness that can't enforce it | `crates/purlis-core/src/personaprofile.rs:220`, `:479-487` | `tools` |
| 96 | Profile list and bypass re-checked on every restart/reopen | `crates/purlis-core/src/dispatchprofiles.rs:48-55` (`may_start_again`, `task_profile_refusal`) | `trust` |

### Harness permission pre-allows for dispatch/handoff

| # | Decision | Where | Family |
|---|---|---|---|
| 97 | `purlis dispatch --name/--to/--profile/--long/--in *`, `dispatch report *` pre-allowed | `crates/purlis-core/src/harness/claude.rs:1577` (`DISPATCH_ALLOW`), `:828` | `tools` |
| 98 | `purlis dispatch --wait/wait/list/read/cancel/tell/note/ask/answer/pass-on/close` pre-allowed | `crates/purlis-core/src/harness/claude.rs:1600` (`DISPATCH_TASK_ALLOW`) | `tools` |
| 99 | `purlis handoff --name/--report/--persona/--create/--vision *`, `handoff report *` pre-allowed | `crates/purlis-core/src/harness/claude.rs:1629` (`HANDOFF_ALLOW`) | `tools` |
| 100 | `dispatch_*` MCP tools run without harness prompt | `crates/purlis-core/src/chattools.rs:614` (`run_on_the_dispatch_grant`); claude `crates/purlis-core/src/harness/claude.rs:846`; codex `approval_mode="approve"` `crates/purlis-core/src/chattools.rs:512`; opencode `crates/purlis-core/src/opencode.rs:383` | `tools` |
| 101 | Dispatch/handoff sharing its call with another writing command, a redirection, or a brief not from one quoted heredoc → denied (no ask) | `crates/purlis-core/src/dispatchguard.rs:819` (`rider_refusal`) | `guards.shape` |
| 102 | Dispatch text (report/tell/answer/pass-on/note/ask) with a live command substitution → denied | `crates/purlis-core/src/dispatchguard.rs:284` (`SENDS_TEXT`), `:364` (`text_refusal`) | `guards.shape` |
| 103 | `purlis handoff --now` from a chat → denied | `crates/purlis-core/src/dispatchguard.rs:497-500` | `tools` |
| 104 | `purlis worktree remove --force` from a chat → denied | `crates/purlis-core/src/dispatchguard.rs:827` | `guards.plane` |

### Handoff

| # | Decision | Where | Family |
|---|---|---|---|
| 105 | Handoff = dispatch in handoff mode; consent is the dispatch grant | `crates/purlis-core/src/handoff.rs:7-11`; `app/src-tauri/src/handoff.rs` (`would_start`) | `dispatch` |
| 106 | Handoff refused from a helper sub-agent | `crates/purlis-core/src/handoffguard.rs:111,155`, `:696` | `guards.shape` |
| 107 | Handoff in another spelling / inside a shell string or heredoc a shell runs / brief not from a quoted heredoc → refused | `crates/purlis-core/src/handoffguard.rs:116-161`, `:276`, `:348`, `:707` | `guards.shape` |
| 108 | First message ≤12 288 bytes; not empty, not `-`-led, not one word, no NUL | `crates/purlis-core/src/handoff.rs:98`, `:739` (`bad_message`); `app/src-tauri/src/handoff.rs` (re-check) | `limits` |
| 109 | Handoff held to the limits of the workspace it moves into | `app/src-tauri/src/handoff.rs:2734-2742` | `limits` |
| 110 | Handoff to another persona into another workspace crosses under the unattended crossing rule | `app/src-tauri/src/handoff.rs:2850-2860` | `dispatch` |

### Persona gate / grant / mark / names

| # | Decision | Where | Family |
|---|---|---|---|
| 111 | Persona tool gate: Bash command whose program is in the persona's `tools:` runs without prompt (never denies) | `crates/purlis-core/src/personagate.rs:850` (`decide`); `crates/purlis-core/src/toolhooks.rs:726`; `crates/purlis-cli/src/guard.rs:164` | `tools` |
| 112 | Gate declines any command with shell metacharacters | `crates/purlis-core/src/personagate.rs:237` (`shell_literal`) | `tools` |
| 113 | Gate declines interpreters/wrappers (sh, bash, python, env, sudo, xargs, find, make…) | `crates/purlis-core/src/personagate.rs:117` (`INTERPRETERS`) | `tools` |
| 114 | Gate declines destructive verbs: kubectl delete/drain/exec…, gh/glab delete/remove/revoke/rotate/transfer, git clean, agentmail send…, purlis/charter/edm secret/vault/change; gh hyphenated deletions; forge `api` non-GET/HEAD | `crates/purlis-core/src/personagate.rs:79` (`DANGEROUS`), `:275`, `:282-285`, `:367` | `tools` |
| 115 | Gate declines anything touching control surface (state dir, vaults, persona definitions) | `crates/purlis-core/src/personagate.rs:459-555` | `fixed` |
| 116 | Tools ceiling frozen at session start; live edit can only narrow; ceiling kept 7 days | `crates/purlis-core/src/personagate.rs:760` (`snapshot`), `:901` | `tools` |
| 117 | Grant must match what was approved in plane-trust fingerprint on this machine | `crates/purlis-core/src/personagate.rs:45-53`; `crates/purlis-core/src/toolhooks.rs:741-748` | `trust` |
| 118 | `borrows:`/`extends:` misspelled or duplicated ⇒ borrows nothing; `tools`/`agent-tools`/`uses` accumulate down chain | `crates/purlis-core/src/personagrant.rs:41`, `:78` | `tools` |
| 119 | Persona names `charter` and plugin name reserved (create refused) | `crates/purlis-core/src/personas.rs:268` (`RESERVED`) | `guards.plane` |
| 120 | Persona custom icon: PNG only, ≤64 KiB, read through containment | `crates/purlis-core/src/personamark.rs:99,110` | `limits` |
| 121 | Window persona create/remove never `--force` (CLI may) | `app/src-tauri/src/personas.rs:12-15` | `consents` |

## Vaults and secrets

| # | Decision | Where | Family |
|---|---|---|---|
| 1 | Which vaults a chat may use: those the registry tags for the persona the app started it as, plus any vault the person allowed for that persona on this machine. Nothing else counts | crates/purlis-core/src/secrets/brokered.rs:1973 (`authorise`), :1816 (`allowed_here`), :1828 (`vaults_of`), :2032 (`listed`); app/src-tauri/src/vaultroute.rs:975; app/src-tauri/src/vaults.rs:368 | `vaults` |
| 2 | A chat with no persona gets no vault at all | crates/purlis-core/src/secrets/brokered.rs:1974 | `vaults` |
| 3 | The persona file's own `vault:` line does not open a vault for a chat, because a chat can edit that file | crates/purlis-core/src/secrets/brokered.rs:1992 | `vaults` |
| 4 | A vault not in the registry is refused with no way forward and no Notice | crates/purlis-core/src/secrets/brokered.rs:1981 (`vault_in`), :1902 (`not_tagged`) | `vaults` |
| 5 | Allow on a refused vault's Notice: lets the persona use that vault in this project on this machine. Only from a refusal the chat actually got; never from a hook line | app/src-tauri/src/vaultroute.rs:473 (`refusals.holds`), :489; crates/purlis-core/src/sandbox/local.rs:1768 (`grant_vault`), :2007 (`revoke_vault`) | `vaults` |
| 6 | Admin lock `vault-grants: false` forbids any untagged-vault Allow and makes earlier grants dormant (not counted by authorise, vaults_of, personas_of or listings) | crates/purlis-core/src/sandbox/policy.rs:988, :784 (`forbids_vault_grants`), :790 (`vault_grants_refused`); used at crates/purlis-core/src/secrets/brokered.rs:1817, :1831, :1856, :2035, :3605; app/src-tauri/src/vaultroute.rs:489; app/src-tauri/src/secretasks.rs:759; app/src-tauri/src/asking.rs:3292, :3348 | `vaults` |
| 7 | Refused vaults kept on Notices per chat: at most 5, oldest dropped | app/src-tauri/src/vaultroute.rs:81 (`AT_MOST_PER_CHAT`) | `vaults` |
| 8 | Keep blocked on a vault Notice hides it for the rest of the chat. It does not take away a tag or a grant that already holds | app/src-tauri/src/vaultroute.rs:110, :831 | `vaults` |
| 9 | Sandboxed chats can neither read nor write vault storage, the fingerprint key or `vault-links.json`, in any project and under any spelling of the state folder | crates/purlis-core/src/sandbox.rs:2172; crates/purlis-core/src/sandbox/seatbelt.rs:859 (`VAULT_TAILS`), :866; crates/purlis-core/src/sandbox/claude.rs:201 | `sandbox` |
| 10 | A sandboxed chat never reads a value directly: `secret get` and `secret cp` are refused for every provider, and no grant changes that | crates/purlis-core/src/secrets/brokered.rs:2228 (`value_refused_in_this_chat`); called at crates/purlis-core/src/secrets/cmd.rs:454 | `secrets` |
| 11 | In a sandboxed chat, vault writes and storage checks belong to the person: `secret set/rm/audit`, `vault add/remove/verify` and their `persona secret` forms are refused before anything is read | crates/purlis-core/src/secrets/persons_own.rs:79 (`refused_in_a_sandboxed_chat`), :67 | `vaults` |
| 12 | In a sandboxed chat, `secret exec` reads nothing itself: it goes over the hook socket and the app resolves and runs it. If no app answers, it runs in-process, except that a keyring-backed vault is refused (the Keychain would prompt on the chat's behalf). If purlis cannot tell whether it is sandboxed, also refused | crates/purlis-core/src/secrets/brokered.rs:371 (`refused_in_this_chat`), :377; called at crates/purlis-core/src/secrets/exec.rs:496, crates/purlis-core/src/secrets/cmd.rs:460, crates/purlis-core/src/secrets/keyring.rs:897, crates/purlis-core/src/secrets/identity.rs:667 | `vaults` |
| 13 | Brokered `secret exec` runs only a tool the persona declares (`[sandbox.personas.<p>].tools`), as armed at chat start, with exactly that tool's vault and hand-over, or else a program under a Run grant. Any other command is refused | crates/purlis-core/src/sandbox/tools.rs:1636 (`declared`); crates/purlis-core/src/secrets/brokered.rs:3197, :3588 | `vaults` |
| 14 | A persona's tools do nothing on a machine until the person there allows the persona's hosts and tools together (digest of the committed lines). Any change to the lines needs a new Allow | crates/purlis-core/src/sandbox/persona.rs:491 (`tools_in_force_here`), :354 (`digest_of`); crates/purlis-core/src/sandbox/tools.rs:1414, :1559 | `trust` |
| 15 | Admin lock `persona-tools: false`: no chat gets a tool shim, and saving a project file that adds a tool is refused. A full `purlis secret exec` typed out is unaffected | crates/purlis-core/src/sandbox/policy.rs:985, :475, :481 (`refuses_tool`) | `tools` |
| 16 | Tool shims put a persona's tools first on PATH, in a folder no chat writes. Claude Code also gets `CLAUDE_ENV_FILE` to put that folder first again | crates/purlis-core/src/sandbox/tools.rs:2820 (`first_on_path`), :2798, :2801 | `vaults` |
| 17 | Names that can never be a tool: shells, interpreters, `env`, `sudo`, `xargs`, anything else that runs what it is told, and purlis itself (by name, prefix and resolved program) | crates/purlis-core/src/sandbox/tools.rs:369 (`REFUSED_NAMES`), :462 (`REFUSED_PREFIXES`), :468, :506 (`refused_program`) | `vaults` |
| 18 | A tool may not hand a value in a variable that steers how programs are found or loaded (PATH, HOME, SHELL, IFS, ENV, BASH_ENV, CDPATH, LD_*, DYLD_*) | crates/purlis-core/src/sandbox/tools.rs:725, :729, :732 | `vaults` |
| 19 | Limits on a tool line: at most 16 values, names up to 64 characters, and no variable twice | crates/purlis-core/src/sandbox/tools.rs:122 (`MOST_VALUES`), :125 | `vaults` |
| 20 | A tool's real program is resolved by the app, never where a chat can write, and is run by absolute path from a PATH without chat-writable folders | crates/purlis-core/src/sandbox/tools.rs:1324 (`resolve`), :1476; crates/purlis-core/src/secrets/program.rs:282 (`Ctx::program`), :247 | `vaults` |
| 21 | A general-purpose program (curl, wget, ssh, gh, cat, base64, openssl…) can still be a tool, but its Allow shows a caution | crates/purlis-core/src/sandbox/tools.rs:663 (`GENERAL_PURPOSE`), :693 (`caution`) | `vaults` |
| 22 | Per-tool argument allowlist `args` (glob per argument; an option matches only a pattern that starts with `-`). Without it, any arguments are allowed | crates/purlis-core/src/sandbox/tools/args.rs:131 (`permits`), :35 (`MOST`=64), :38 (`LONGEST`=256) | `vaults` |
| 23 | Forge CLI subcommands that show or change credentials or settings never run with a vault value, whatever `args` says | crates/purlis-core/src/sandbox/tools/args.rs:272 (`refused_forge`) | `vaults` |
| 24 | A brokered run's environment: only a fixed allowlist of the chat's variables (PATH, LANG, TZ, TERM, SSH_AUTH_SOCK, CA bundles, PG*/MYSQL/REDIS host and port, AWS region, LC_*) | crates/purlis-core/src/sandbox/tools.rs:2698 (`A_RUN_STARTS_FROM`), :2732 | `vaults` |
| 25 | Never passed to a run: hook socket, token and prompt key; code-loading variables (NODE_OPTIONS, PYTHONPATH, LD_*…); crash-dump variables (GOTRACEBACK, GODEBUG) | crates/purlis-core/src/secrets/brokered.rs:3070 (`NOT_HANDED_ON`), :3377 (`may_hand_on`), :3385 (`DUMPS_MEMORY`); crates/purlis-core/src/sandbox/tools.rs:2647 (`LOADS_CODE`), :2680 | `vaults` |
| 26 | A run under a Run grant also drops connection and CA variables, so where it connects is said in the command the person read | crates/purlis-core/src/sandbox/tools.rs:2740 (`A_GRANTED_RUN_LEAVES`) | `vaults` |
| 27 | No core dumps for brokered runs (RLIMIT_CORE soft limit set to 0) | crates/purlis-core/src/secrets/brokered.rs:3391 | `vaults` |
| 28 | A brokered run is sandboxed with the chat's own compiled confines: same denials, writes only to the chat folder and its own temp, network only through the egress proxy with the chat's hosts. On a system with no wrap, nothing runs | crates/purlis-core/src/secrets/brokered.rs:2303 (`runs_a_chats_command_here`), :1789 (`NO_WRAP`) | `vaults` |
| 29 | On Linux, a Claude Code chat under its own sandbox may not use `secret exec --env`; it must use `--file` | crates/purlis-core/src/secrets/brokered.rs:2287 (`env_refused`), :2277; called at :3557 | `vaults` |
| 30 | A host the run's proxy refused that contains one of the run's values is never named and never offered for Allow | crates/purlis-core/src/secrets/brokered.rs:3019 (`carries_a_value`), :3029 (`WITHHELD_NOTE`) | `secrets` |
| 31 | Brokered I/O limits: frame ≤256 KiB, ≤128 KiB of output per frame, each passed variable ≤4 KiB, pipes read 2 s after exit, a "slow" note after 3 s | crates/purlis-core/src/secrets/brokered.rs:271, :276, :282, :285, :292 | `limits` |
| 32 | How long `secret exec` waits on the app: ¾ of the harness's default command timeout + 1 s (91 s on Claude Code and opencode, 8.5 s on Codex), or 180 s for an unknown harness | crates/purlis-core/src/secrets/brokered.rs:308 (`of_the_app`), :297; crates/purlis-core/src/secrets/ask.rs:516 (`default_command_timeout`), :1996 | `limits` |
| 33 | `vault list` from a chat waits 5 s; `secret list <vault>` waits 30 s | crates/purlis-core/src/secrets/brokered.rs:2075, :2084 | `limits` |
| 34 | Run grants (ADR 0112): a program the persona does not declare can be allowed by the person to receive the keys its command names, from one vault. Allow once binds the exact command | crates/purlis-core/src/secrets/brokered.rs:1498 (`run_granted`); crates/purlis-core/src/secrets/ask/run.rs:682 (`allowed_or_held`), :1013 (`answer_run`) | `vaults` |
| 35 | Admin lock `run-grants: false` (or `persona-tools: false`): no Allow is raised and no earlier grant is used. A missing key can still be filled | crates/purlis-core/src/sandbox/policy.rs:993, :765, :772 (`refuses_run_grant`); crates/purlis-core/src/secrets/brokered.rs:1013 (`run_grant_locked`); app/src-tauri/src/asking.rs:3290 | `vaults` |
| 36 | A Run grant is also refused when `allow-scopes` leaves no usable scope, or when `undeclared-secret-asks: false` and a key named is not declared by the persona | crates/purlis-core/src/secrets/brokered.rs:1022-1043 | `vaults` |
| 37 | Allow for this chat and Always allow (any arguments) only for programs reviewed for any arguments; today only `curl`. Everything else gets Allow once on the exact command | crates/purlis-core/src/sandbox/tools.rs:2222 (`REVIEWED_FOR_ANY_ARGUMENTS`), :2225; app/src-tauri/src/asking.rs:3459 (`program_options`) | `vaults` |
| 38 | Programs never offered a Run grant (they run others: find, fd, git, tmux, screen, crontab, nsenter…) | crates/purlis-core/src/sandbox/tools.rs:1736 (`NOT_FOR_A_RUN_GRANT`) | `vaults` |
| 39 | Flags that name a command are never offered a Run grant (ssh -o ProxyCommand/-F, rsync -e, kubectl --kubeconfig, psql -f, openssl -engine…) | crates/purlis-core/src/sandbox/tools/command_flags.rs:276 (`PROGRAMS`), :78 | `vaults` |
| 40 | A declared tool with an `args` line is never widened by a Run grant; a teammate's or a locked tool is refused as before | crates/purlis-core/src/sandbox/tools.rs:1722 (`limits_its_arguments`), :1707 (`is_declared`); crates/purlis-core/src/secrets/brokered.rs:1541 | `vaults` |
| 41 | Open Run-grant asks per chat: at most 3 | crates/purlis-core/src/secrets/ask/run.rs:51 (`MOST_OPEN_PER_CHAT`) | `limits` |
| 42 | Run grant scope: in memory, for the chat's life. Don't allow is remembered per chat, program and vault. Only Always allow writes a tool line (and gives the persona the vault). Only the window answers or revokes | crates/purlis-core/src/secrets/ask/run.rs:1019, :981 (LocalUi checks); app/src-tauri/src/secretasks.rs:763 (`declare_the_tool`), :751 (`allows_its_own_vault`) | `vaults` |
| 43 | Keys that browser logins, secret-fill lines or `mint` lines hold are never handed to any program a person allows (fails closed if the project file is unreadable) | crates/purlis-core/src/secrets/brokered.rs:1358 (`no_program_is_handed`), :1368; crates/purlis-core/src/secretfill.rs:145 (`a_browsers`), :164 (`a_mints`) | `vaults` |
| 44 | Linked entries are judged when they are read: a link into a vault the persona may not use is refused for every reader (tool, run grant, fill, login, profile). Fails closed if the link or grant file cannot be read | crates/purlis-core/src/secrets/brokered.rs:1390 (`read_out_of_reach`), :1440 (`read_judged`) | `vaults` |
| 45 | Making a link: one hop only (no link to a link, and a key others link to cannot become a link). A link is refused if any persona of the source vault may not use the target vault | crates/purlis-core/src/secrets/links.rs:240 (`link_over`), :221; crates/purlis-core/src/secrets/brokered.rs:1873 (`read_through_by`); crates/purlis-core/src/secrets/ask.rs:1300; app/src-tauri/src/secretasks.rs:455 | `vaults` |
| 46 | A GitHub App's private key (`mint` line) is never read raw; only the mint reads it. Keys with the reserved prefix `agent-identity-github-` are never filled | crates/purlis-core/src/sandbox/tools/mint.rs:160 (`raw`), :41, :44 (`reserved`); crates/purlis-core/src/secrets/cmd.rs:79 (`get_value`) | `vaults` |
| 47 | Secret ask: a brokered run's missing key becomes an ask that only the person answers, in the window. Chats, `charter inbox` and relayed links are refused as answerers | crates/purlis-core/src/secrets/ask.rs:1311 (`answer_as`, LocalUi check at :1319) | `vaults` |
| 48 | `secret_ask` tool / `purlis secret ask`: the key must be a valid name (≤128 bytes, `[A-Za-z_][A-Za-z0-9_.-]*`), a purpose is required (≤200 chars, masked), the chat must have a persona, and it works only in a chat purlis started | crates/purlis-core/src/secrets/ask.rs:1773, :1777, :1780, :113, :2030; crates/purlis-core/src/secrets/needs.rs:34, :22; crates/purlis-core/src/chattools.rs:142 | `vaults` |
| 49 | Which vault an ask goes to: the line's vault, else the persona's vault, else a new keyring vault named after the persona and tagged for it, made at the first fill | crates/purlis-core/src/secrets/ask.rs:1787, :1940 (`made_if_missing`), :1935 (`MADE_ON`) | `vaults` |
| 50 | A fill never replaces a value that is held, and only goes into a vault the persona may use. A replace needs a reason and a usable vault | crates/purlis-core/src/secrets/ask.rs:1917 (`fillable`), :1797-1810 | `vaults` |
| 51 | A key the vault already holds answers "filled" at once, but only when the persona may use that vault (otherwise no key names leak) | crates/purlis-core/src/secrets/ask.rs:1987 (`holds_already`), :1813 | `vaults` |
| 52 | Admin lock `undeclared-secret-asks: false`: an ask for a key the persona does not declare (`secret-need.<KEY>` in persona.md) is declined as `policy` and the person is never asked | crates/purlis-core/src/sandbox/policy.rs:992, :760; crates/purlis-core/src/secrets/ask.rs:607 (`refused_by_policy`), :616; crates/purlis-core/src/secrets/brokered.rs:1102 (`fills_refused`) | `vaults` |
| 53 | Declared Secret needs are read from committed HEAD down the `extends` chain (≤16 ancestors, ≤256 KiB per definition) | crates/purlis-core/src/secrets/needs.rs:52, :93, :96, :120 | `vaults` |
| 54 | How long an ask holds a waiting command: ¾ of the harness command timeout (10 s base for an unknown harness). The `secret ask` CLI waits 15 s past that | crates/purlis-core/src/secrets/ask.rs:529 (`holds_for`), :509, :1996 | `limits` |
| 55 | One open ask per (vault, key) per project; a decline covers the chats waiting at that moment, for their lives; asks survive app restarts (`secret-asks.json`) | crates/purlis-core/src/secrets/ask.rs:853 (`hold`), :76 (`KEPT_IN`) | `vaults` |
| 56 | A chat on a pool runner that cannot receive values is declined with "runner can't receive secrets". Values reach a runner only through its run's own Allow (`secret.forwarded`) | crates/purlis-core/src/secrets/ask.rs:1047 (`cannot_receive`); crates/purlis-core/src/secrets/brokered.rs:3415, :3351 (`Handed::is_for`) | `vaults` |
| 57 | Browser secret fill (`browser_fill_secret`): names only. Refused for a mint-reserved key, an unreadable project file, an unregistered vault or a link out of reach. An untagged vault is warned about on the notice, or refused under `vault-grants: false` | crates/purlis-core/src/secretfill.rs:27 (`judged`); crates/purlis-core/src/browsertools.rs:248 | `vaults` |
| 58 | Fill answers: Allow once, Allow for this chat (in memory), Always allow (writes a `fill = "<origin>"` line in the persona's tools), Keep blocked. Always is held back when `proposed-changes = "off"` and not offered for a chat with no persona or for a 2FA code | app/src-tauri/src/secretfills.rs:768 (`granted`), :800, :804; crates/purlis-core/src/secretfill.rs:344 (`line`) | `vaults` |
| 59 | A grant holds only `https://` origins open; `http://localhost` and other local sites are asked about every time. Fill-line origins must be https, or http only on localhost/127.0.0.1/[::1] | app/src-tauri/src/secretfills.rs:795 (`held_open`); crates/purlis-core/src/sandbox/tools/fill.rs:44 (`origin_of`) | `vaults` |
| 60 | Always asked: card keys (`card = true` on any persona's line, failing closed), fields marked for payment (`autocomplete=cc-*`), multi-line or editable fields, and fields whose kind is unknown | app/src-tauri/src/secretfills.rs:751 (`asks`), :113 (`once_only`); crates/purlis-core/src/secretfill.rs:86 (`lines`); crates/browser-engine/src/drive/browser/fill.rs:646 (`takes`) | `vaults` |
| 61 | Under a grant, a field whose form submits by GET is refused (Allow once still works) | app/src-tauri/src/secretfills.rs:611 | `vaults` |
| 62 | After 2 Keep blocked per persona (or per chat with no persona), its chats are asked about no further fills until purlis restarts. Only one fill notice may be pending per chat | app/src-tauri/src/secretfills.rs:233 (`KEPT_AT_MOST`), :236 | `vaults` |
| 63 | The engine judges again at fill time: the field's own frame must be on the approved origin, no related window may be on the site, the browser is held to the site while the page lives, and values shorter than 3 characters are refused | crates/browser-engine/src/drive/browser/fill.rs:280 (`fill_secret`), :330-360, :689; crates/browser-engine/src/drive/scrub.rs:30 | `vaults` |
| 64 | No private key, no `otpauth://` address and no seed-shaped value is ever filled into a page. A seed-shaped value is allowed only if the person marked it as no seed in the window; a 2FA code can be filled instead | crates/purlis-core/src/secrets/brokered.rs:2623 (`goes_into_a_page`), :2607, :2677 (`fill_code`); crates/purlis-core/src/secrets/no_seed.rs:182 (`mark`); crates/purlis-core/src/secrets/totp.rs:106, :82 | `secrets` |
| 65 | Browser login tool: the chat picks only which declared login to use. The page, origin and values come from the committed line and the vault, and the login must be armed and its vault granted | crates/purlis-core/src/secrets/brokered.rs:2830 (`login_allowed`), :2864; crates/purlis-core/src/sandbox/tools/login.rs:34 | `vaults` |
| 66 | Credentialed MCP servers get the vault only through `secret exec`, and only after this machine approves the entry's fingerprint in a terminal (`purlis persona approve-mcp`). Otherwise the server is withheld. Codex and opencode do not start them | crates/purlis-core/src/personaverbs/chatstart.rs:68 (`servers`), :117; crates/purlis-core/src/personaverbs/mcp.rs:355 (`approved`), :374 (`approve`); crates/purlis-core/src/secrets/brokered.rs:2469 | `vaults` |
| 67 | A credentialed MCP server that fails to start is retried once after 2 s; up to 256 KiB of stdin is kept for the retry; the error shown is at most 400 characters and masked | crates/purlis-core/src/secrets/mcpstart.rs:38, :42, :45, :351 | `vaults` |
| 68 | `secret get` shows a mask and fingerprint. `--reveal` is refused to non-terminal stdout unless `--force` | crates/purlis-core/src/secrets/cmd.rs:470, :490 | `secrets` |
| 69 | `secret cp` refuses devices, FIFOs, directories, symlinks and paths git would commit. Writing a plain-file vault refuses a path git would commit | crates/purlis-core/src/secrets/cmd.rs:882 (`cp_dest_refusal`), :945, :640 (`plaintext_refusal`); crates/purlis-core/src/secrets/vaultcmd.rs:152 | `vaults` |
| 70 | `secret set` refuses an empty value unless `--allow-empty` | crates/purlis-core/src/secrets/cmd.rs:664 | `vaults` |
| 71 | `secret exec --exec` cannot be combined with `--file`/`--dotenv`, nor with `--stream` | crates/purlis-core/src/secrets/exec.rs:469, :486 | `vaults` |
| 72 | Output redaction: every value resolved is replaced in captured output (longest first). `--stream`/`--exec` are not redacted | crates/purlis-core/src/secrets/mod.rs:354 (`redact`); crates/purlis-core/src/secrets/exec.rs (module doc) | `secrets` |
| 73 | Values read or stored in this process are remembered as HMAC digests so that stores of a chat's words can mask them (values of ≤3 characters are not remembered). If no key can be made, every text counts as holding a value | crates/purlis-core/src/secrets/seen.rs:39 (`SHORTEST`), :222 (`holds`), :246 (`found_in`) | `secrets` |
| 74 | Every route by which a value leaves (exec, cp, get --reveal) writes one trace event that names the vault and keys, never a value | crates/purlis-core/src/secrets/cmd.rs:361 (`trace_secret_use`), :349 | `secrets` |
| 75 | New vault provider defaults to keyring. Providers are fixed: 1password, keyring, plain-file, reference | app/src-tauri/src/vaults.rs:1614 (`DEFAULT_PROVIDER`); crates/purlis-core/src/secrets/registry.rs:24 (`PROVIDERS`) | `vaults` |
| 76 | Registering a vault is local by default (`.charter/vaults.json`); `--share` publishes it to the committed `vaults.json`. `account` and `identity` stay local only | crates/purlis-core/src/secrets/registry.rs:29 (`LOCAL_ONLY_KEYS`); crates/purlis-core/src/secrets/vaultcmd.rs:317 | `vaults` |
| 77 | Keyring items are held to the app's code signature (the app binary creates them), so any other program, a chat's included, prompts or fails | crates/purlis-core/src/secrets/keyhold.rs:435 (`make_own`), :468, :59 (`PATIENCE` 20 s), :56 (1 MiB input) | `vaults` |
| 78 | A vault identity token (OP service account) is kept in the keyring, pinned to its binding. The `op` it runs must not be anywhere a chat writes and, on macOS, must carry 1Password's signature | crates/purlis-core/src/secrets/identity.rs:831 (`pinned_op`), :172 (`ONEPASSWORD_TEAM_ID`), :421; crates/purlis-core/src/secrets/program.rs:319 (`judge_pinned`) | `vaults` |
| 79 | No `OP_*` variable of purlis's own environment reaches a provider CLI; only the vault's `env` binding supplies one. `OP_*` is kept from chats | crates/purlis-core/src/secrets/run.rs:62 (`from_purlis`), :36, :43; crates/purlis-core/src/secrets/identity.rs:1334, :1340 | `vaults` |
| 80 | Reference (`op://`, `vault://`) resolution times out after 60 s, and resolver output is withheld from errors | crates/purlis-core/src/secrets/reference.rs:32, :111 | `vaults` |
| 81 | Plain-file vault files are 0600; any group/other bits on a vault directory are flagged | crates/purlis-core/src/secrets/plain_file.rs:103 (`tighten`), :148; crates/purlis-core/src/secrets/mod.rs:414 (`OTHERS`) | `vaults` |
| 82 | Before the sandbox is turned off, plain-file vaults move into the keychain (all or nothing). A plain-file vault registered by the team file refuses that setup | crates/purlis-core/src/secrets/tokeyring.rs:75; crates/purlis-core/src/sandbox/personal.rs:405 (`chosen_at_setup`) | `vaults` |
| 83 | 1Password set-up holds a token for at most 15 min | app/src-tauri/src/vaults.rs:1125 | `vaults` |
| 84 | Vault references in saved text (`vault:forge/gh`) do not count as credentials for the save/memory secret scan (narrow: name slots only, ≤32 names) | crates/purlis-core/src/secretshape.rs:84 (`REFERENCE_SLOT`), :89, :839 (`secret_kind`) | `guards.save` |

## The browser

| # | Decision | Where | Family |
|---|---|---|---|
| B1 | Whether a chat may have a browser at all: its first browser call asks the person once (This project's sites / Also the open web / Keep off). Nothing starts until they answer, and every call made meanwhile is refused | `app/src-tauri/src/chatbrowsers.rs:3396` (`answer_now`, Posture::Unasked→Asked); `Posture` at :206 | `browser` |
| B2 | Keep off refuses every browser call the chat makes, and tells it not to retry | `app/src-tauri/src/chatbrowsers.rs:3404` (`KEPT_OFF` :987) | `browser` |
| B3 | "Always for this persona" answers the browser Ask for every chat of that persona in this project on this machine (project sites, or the open web too) | `app/src-tauri/src/chatbrowsers.rs:3390` (applies it), :5401 `allowed_always`; kept by `crates/purlis-core/src/sandbox/local.rs:1861` | `browser` |
| B4 | A persona's declared `browser = "project" \| "open-web"` only pre-selects an answer on the Ask and never answers it | `crates/purlis-core/src/sandbox/persona.rs:133` `declared_browser`; marked in `app/src-tauri/src/asking.rs:2406` `browser_posture` | `browser` |
| B5 | A chat with no persona is never offered Always, and is refused it | `app/src-tauri/src/chatbrowsers.rs:5419` | `browser` |
| B6 | What a chat's browser lists: the chat's compiled sandbox reach (never the floor), plus the browser sites you allowed here, plus the project's committed browser sites already shown here, plus the chat's own sites | `app/src-tauri/src/chatbrowsers.rs:5184` `reach_of`, :5225 `with_browser_sites`, :5990 `with_sites` | `browser` |
| B7 | A chat started without the sandbox gets a browser that lists what a sandboxed chat of its persona would list, read at each call | `app/src-tauri/src/chatbrowsers.rs:5190` → `crates/purlis-core/src/sandbox/browser_listing.rs:21` `listed_for` | `browser` |
| B8 | With the open web off, the reach proxy refuses any site that nothing lists; with no reach given (the person's browser) every new site is refused | `crates/purlis-core/src/browsers.rs:757` `reach_of` (`NewSites::refusing`/`carrying`) | `browser` |
| B9 | Site judge: a page's own reads (links, images, scripts) of a new public site go; an address the agent writes, a form or body sent, or typing on a page of a new site asks the person; with the open web on nothing asks; what the person does while driving goes | `crates/purlis-core/src/browsers/judge.rs:231` `SiteJudge::judge` | `browser` |
| B10 | Most new-to-project sites one browser's proxy lets on (and most leave-asks it keeps) | `crates/purlis-core/src/browsers/judge.rs:54` `MOST_LET_ON` | `browser` |
| B11 | Answers offered on a browser-sites Ask: Allow for this chat / Always allow in this project on this machine / Allow for everyone in this project / Keep blocked, each unless policy removes its scope; sites not named whole get Keep blocked only | `app/src-tauri/src/asking.rs:3548` `browser_site_answers` | `browser` |
| B12 | A browser-site Allow is for browsers alone: it never lets the chat's own commands reach that host | `app/src-tauri/src/chatbrowsers.rs:5657` `sites_allowed` (doc :5652); `crates/purlis-core/src/sandbox/local.rs:1944` `grant_browser_site` | `browser` |
| B13 | A browser-site Allow is refused at a level other than chat/you/project, or where policy removes that scope or never allows the host | `app/src-tauri/src/chatbrowsers.rs:5674` (levels), :5722 `sites_grantable` | `browser` |
| B14 | "Allow for everyone in this project" (a committed browser site) is refused where the project's sandbox is not in force | `app/src-tauri/src/chatbrowsers.rs:5868` `everyone_counts`, :5731 | `browser` |
| B15 | Committed browser sites (`[sandbox] browser-sites`) reach a browser on a machine only after the person there was shown each one in the Notice; filtered by `never-hosts` and `allow-scopes` | `crates/purlis-core/src/sandbox/browser_sites.rs:76` `in_force`, :44 `committed` | `browser` |
| B16 | Browser sites you allowed on this machine are held (kept but unused) while policy removes `you` or never allows the host | `crates/purlis-core/src/sandbox/local.rs:1932` `browser_sites_in_force` | `browser` |
| B17 | Most sites one chat's browser waits on the person for at once (asked and answered lists) | `app/src-tauri/src/chatbrowsers.rs:969` `MOST_SITES_ASKED` | `browser` |
| B18 | When the person narrows a chat's reach, all of that chat's browsers end; the next call opens one on the reach as it is then | `app/src-tauri/src/chatbrowsers.rs:3097` `narrowed` | `browser` |
| B19 | Browser reach proxy, by host and port: only public addresses; loopback only as `127.0.0.1:N`/`[::1]:N` listed exactly; a private address only listed exactly with its port; `localhost`, `*.localhost`, a name of this Mac, or a name ending in a number is refused | `crates/purlis-core/src/sandbox/browser_reach.rs:155` `Lane::decide`, :198 `local` | `browser` |
| B20 | A new site reaches the proxy only on 443 (CONNECT) or 80 (plain); any other port needs an exact Allow | `crates/purlis-core/src/sandbox/browser_reach.rs:186` (uses `TUNNEL_PORTS`/`PLAIN_PORTS`, `crates/purlis-core/src/sandbox/egress.rs:92`, :95) | `browser` |
| B21 | A host that policy pins as never allowed is refused to a browser whatever lists it | `crates/purlis-core/src/sandbox/browser_reach.rs:190` (`Refusal::Policy`) | `browser` |
| B22 | Fail closed where this machine's addresses can't be read | `crates/purlis-core/src/sandbox/browser_reach.rs:128` `decide_here` | `browser` |
| B23 | Proxy name lookups: at most 32 running across all proxies, each given up after 10 s | `crates/purlis-core/src/sandbox/browser_reach.rs:693` `LOOKUPS_AT_ONCE`, :690 `LOOKUP_PATIENCE` | `browser` |
| B24 | A browser holding a login is bound to that login's site (and its form's site) for the rest of its life; every other connection is ended and refused, and no Allow widens it | `crates/purlis-core/src/sandbox/browser_reach.rs:531` `Bound::to`; judge skip at `crates/purlis-core/src/browsers/judge.rs:243` | `browser` |
| B25 | The agent is kept off the login's sign-in site except during a sign-in (it reads and acts on nothing there) | `crates/purlis-core/src/browsers.rs:1793` `kept_off_at`; `crates/purlis-core/src/sandbox/browser_reach.rs:664`; engine `crates/browser-engine/src/drive/mod.rs:821` `Refusal::KeptOff` | `browser` |
| B26 | While a page holding a filled secret lives, the browser reaches only that site (plus the sites its key is approved for); a fill on another site meanwhile is refused | `crates/purlis-core/src/sandbox/browser_reach.rs:460` `hold_for_fill_also`; agent address refused at `crates/purlis-core/src/browsers.rs:1439` `held_to_said` | `browser` |
| B27 | A filled page may leave for another site only by its server's redirect: on its own to where the chain began, asked once for any other, and never where the usual reach refuses or a login keeps it off | `crates/purlis-core/src/browsers/judge.rs:178` `leaving`; engine `crates/browser-engine/src/drive/browser/leaving.rs:759` `carries_value` refuses an address holding the value | `browser` |
| B28 | Only http/https addresses reach the engine: other schemes, user:pass@, whitespace, control chars and backslashes are refused | `crates/browser-engine/src/address.rs:40` `WebAddress::parse`; `crates/purlis-core/src/browsers.rs:788` `call_of`; person-typed `crates/purlis-core/src/browsers/typed.rs:27` | `browser` |
| B29 | No browser tool runs script; the agent's text reaches a page only as typed text or as text to find/wait for | `crates/purlis-core/src/browsertools.rs:9` (tool set `TOOLS` :876) | `browser` |
| B30 | Browser tools a harness runs without its own prompt: every tool except `browser_upload` and `browser_save_download`, which the harness asks about each time | `crates/purlis-core/src/browsertools.rs:94` `ASKED_EACH_TIME`, :83 `run_on_the_browser_ask`; written into Claude's allow list at `crates/purlis-core/src/harness/claude.rs:870` | `browser` |
| B31 | A grade's (eval) chat is listed and answered no browser tools | `crates/purlis-cli/src/mcp.rs:93`, :109 (`chattools::PRE_ALLOWED`) | `browser` |
| B32 | Most browsers the chats of one project hold at once (chat-opened, handed over, opened for a login or a suggested page) | `crates/purlis-core/src/browserlimits.rs:11` `PER_PROJECT`; enforced `app/src-tauri/src/chatbrowsers.rs:3420`, :1948 (hand over), :2490 (suggested page), :1412 (login) | `browser` |
| B33 | Most browsers this device runs at once: one per 4 GiB of the memory class, never zero; 16 GiB class if memory can't be read | `crates/purlis-core/src/browserlimits.rs:166` `budget_for` (:19 `GIB_PER_BROWSER`, :23 `UNREAD_CLASS_GIB`); enforced by the session host `crates/host/src/hosting.rs:4618` `PlaceInTheBudget::taken` | `browser` |
| B34 | A chat's browser stops after this long unused (not when pinned, driven by the person, or waited on by an Ask) | `crates/purlis-core/src/browserlimits.rs:14` `IDLE_AFTER`; `app/src-tauri/src/chatbrowsers.rs:1091` `idle`, swept every 30 s (:6187) | `browser` |
| B35 | Only the window sets browser limits (never a chat or the CLI) | `app/src-tauri/src/browserlimits.rs:120` `set_browser_limit` (`ui::WINDOW_ONLY`) | `browser` |
| B36 | Which engine runs: this machine's Chrome if the person chose it and it is installed, else the pinned Chrome for Testing if it can make its sandbox, else none; never an engine without its sandbox | `crates/purlis-core/src/browsers.rs:734` `engine_for` | `browser` |
| B37 | Pinned engine on Linux runs only under an AppArmor profile that names it, where AppArmor restricts user namespaces | `crates/purlis-core/src/browsers/sandboxing.rs:109` `sandboxes` | `browser` |
| B38 | Engine age: runs with a warning 7 days behind and is refused 21 days behind; `run_outdated` exists but nothing sets it | `crates/browser-engine/src/install.rs:489` `EngineSettings::default` (:670 refusal); core passes defaults at `crates/purlis-core/src/browsers.rs:402` | `browser` |
| B39 | Engine update: a manifest is taken only when its Sigstore bundle verifies against the manifest workflow's identity and GitHub's issuer; checked 5 min after start and every 6 h; read within 60 s | `crates/browser-engine/src/sigstore.rs:108` `verify` (:43 `ISSUER`); `crates/purlis-core/src/browsers.rs:196`/:199/:202 | `browser` |
| B40 | Engine download limits: archive at most 1 GiB, manifest at most 1 MiB, 5 redirects, 2 retries on a server error, an hour for the archive | `crates/browser-engine/src/install.rs:55`, :69, :264, :269, :289 | `browser` |
| B41 | The first download of the engine can be cancelled by the person, which refuses that start | `app/src-tauri/src/enginedownload.rs:158` `cancel_engine_download` | `browser` |
| B42 | Engine launch flags: DevTools only over a pipe (no port), headless, every request through the reach proxy (loopback too), no DNS of its own, QUIC off, no non-proxied WebRTC UDP, crash reports, sync, extensions and background networking off, no back-forward cache | `crates/browser-engine/src/launch.rs:145` `arguments` | `browser` |
| B43 | Password saving, leak detection and autofill are off in every engine data folder | `crates/browser-engine/src/launch.rs:233` | `browser` |
| B44 | Only these environment variables reach the engine: PATH, TMPDIR, LANG, LC_ALL, LC_CTYPE, TZ, USER, LOGNAME | `crates/browser-engine/src/launch.rs:80` `ENVIRONMENT_KEPT` | `browser` |
| B45 | Page size frames are painted at | `crates/browser-engine/src/launch.rs:38` `PAGE_SIZE` | `browser` |
| B46 | Driver limits: settle 10 s after 500 ms quiet; snapshot 300 lines by default, 2000 at most; a text wait 60 s at most; page tree read 60 s; tree read at most 2× the CDP message cap; login form wait 30 s | `crates/browser-engine/src/drive/mod.rs:1072` `Limits::default` (`LOGIN_WAIT` at `drive/browser/login.rs:90`) | `browser` |
| B47 | `browser_wait_for_text` waits 10 s when the call says nothing; the MCP tool waits 90 s on the app; the window waits 85 s for an act and 150 s for a login | `crates/purlis-core/src/browsertools.rs:37` `WAIT_SECONDS`; `crates/purlis-cli/src/browsing.rs:17` `ASKED_WITHIN`; `app/src-tauri/src/browsers.rs:296`, :300 | `browser` |
| B48 | A screenshot the agent gets is at most 600 KiB of PNG, halved up to 4 times to fit, then refused | `crates/purlis-core/src/browsers.rs:751` `MOST_SCREENSHOT_BYTES`; `crates/browser-engine/src/drive/fit.rs:12` `MOST_HALVINGS` | `browser` |
| B49 | Downloads: held in the browser's own folder (no chat reads it); one download at most 512 MiB, 64 per browser, 1 GiB held together; past that, cancelled | `crates/purlis-core/src/browsers.rs:122` `DOWNLOADS`; `crates/browser-engine/src/drive/mod.rs:1012`, :1015, :1018 | `browser` |
| B50 | Saving a download into the chat's folder needs the person's yes for its origin, and no prompt exists yet, so every save is refused | `app/src-tauri/src/browserfiles.rs:184` (allowed list only set in tests, :117) | `browser` |
| B51 | Where a download may be saved (once allowed): a relative path in the chat's folder, nothing purlis keeps from chats, passing the brokered-write guard, written as a new file following no link | `app/src-tauri/src/browserfiles.rs:220` `writable`; `crates/purlis-core/src/browserfiles.rs:215` `bring` | `browser` |
| B52 | Uploads: only from the chat's folder by relative path, no links, a plain file with one name, nothing purlis keeps from chats; at most 50 MiB a file, 20 files and 100 MiB a call | `crates/purlis-core/src/browserfiles.rs:35`, :38, :41, :155 `stage`; `app/src-tauri/src/browserfiles.rs:206` `readable` | `browser` |
| B53 | A runner chat's browser neither uploads nor saves | `app/src-tauri/src/browserfiles.rs:138`, :157 (`NOT_ON_A_RUNNER` :47) | `browser` |
| B54 | `browser_login` fills only a login the persona declares and this machine allowed (as armed at the chat's start, or read at the call for an unsandboxed chat); the chat chooses which login and nothing else | `crates/purlis-core/src/secrets/brokered.rs:2528` `login_for`, :2858 `login_ready` | `browser` |
| B55 | A declared login page must be https, or http only for localhost/127.0.0.1/[::1]; no user:pass in it | `crates/purlis-core/src/sandbox/tools/login.rs:85` `origin_of` (:155 http rule) | `browser` |
| B56 | A login the persona doesn't declare raises a notice (Allow once / for this chat / Always / Keep blocked); waits 15 min; a suggested site must be https and not an address or single-label name | `crates/purlis-core/src/loginask.rs:213` `site_of`, :43 `THE_PERSON_IS_WAITED_FOR`; `app/src-tauri/src/loginasks.rs` (module) | `browser` |
| B57 | Always on a login or fill notice is withheld where policy sets `proposed-changes = "off"`, removes `you`, the project has no `[sandbox]`, or the persona's list isn't seen yet | `app/src-tauri/src/loginasks.rs:887` `always_held` | `browser` |
| B58 | A chat whose suggested login pages fail to be followed 3 times within 10 min has none followed for 10 min | `app/src-tauri/src/chatbrowsers.rs:326` `FOLLOWS_FAILED_AT_MOST`, :330 `FOLLOWS_REST` | `browser` |
| B59 | A value that is a private key, an otpauth address, or looks like a 2FA seed (unless the person marked it as no seed) never goes into a page | `crates/purlis-core/src/secrets/brokered.rs:2623` `goes_into_a_page` | `browser` |
| B60 | `browser_fill_secret`: plain vault/key names only; a key a `mint` line names (or Agent identity's) is never filled; nothing is filled while the project file can't be read; a link into a vault out of the persona's reach is refused; an untagged vault is a warning, or refused under `vault-grants` | `crates/purlis-core/src/secretfill.rs:27` `judged` | `browser` |
| B61 | A fill goes in without a notice only under a grant (an allowed `fill` line for that key and origin, or Allow for this chat), only on an https origin, only into a one-line field the page doesn't mark for payment, whose form doesn't send by GET; otherwise the person is asked | `app/src-tauri/src/secretfills.rs:750` `asks`, :765 `granted`, :795 `held_open`, :611 (GET) | `browser` |
| B62 | Once-only answers: a card's key (`card = true` on any persona's line, failing closed), a payment field (`autocomplete="cc-…"`), a multi-line or editable field, or a non-https origin get only Allow once / Keep blocked; a 2FA code never gets Always | `app/src-tauri/src/secretfills.rs:113` `once_only`, :804; `crates/purlis-core/src/secretfill.rs:86` `lines` | `browser` |
| B63 | After 2 Keep blocked, a persona's chats (a no-persona chat: that chat) are asked about no more fills until purlis restarts | `app/src-tauri/src/secretfills.rs:233` `KEPT_AT_MOST` | `browser` |
| B64 | At the moment of filling the engine re-checks the frame's origin and the field; a filled field takes no keys, and the agent copies, cuts and drags nothing in that browser for its life | `crates/browser-engine/src/drive/browser/fill.rs:280` `fill_secret`, :649 (multi-line), :139 (copy/cut/drag stop) | `browser` |
| B65 | A login on a pool's runner needs the person's opt-in, bound to the pool's definition, image, runner and the login's identity; only a runner shown to be the person's own receives a value; held in memory for the app's run | `crates/purlis-core/src/poollogins.rs:432` `owned`; `app/src-tauri/src/chatbrowsers.rs:520` `PoolLoginWaits`, `answer_pool_login` :5571 | `browser` |
| B66 | A browser that performed a declared login keeps it (sealed, this device only) for the next chat of the same profile/persona/login identity, whenever the chat has a profile | `app/src-tauri/src/chatbrowsers.rs:4956` (`login.keep`); `crates/purlis-core/src/browserlogins.rs:168` `identity_of` | `browser` |
| B67 | A kept login store holds at most 256 MiB unsealed | `crates/purlis-core/src/browserlogins.rs:67` `MOST_BYTES` | `browser` |
| B68 | A browser signed in with a login is handed over only to a chat whose login has the same identity now | `app/src-tauri/src/chatbrowsers.rs:6095` `may_log_in`; `hand_over` :1889 | `browser` |
| B69 | A chat names and closes only its own browsers; it can't close one the person drives | `app/src-tauri/src/chatbrowsers.rs:3514` (Close), :3523 | `browser` |
| B70 | While the person drives a browser, every agent call on it is refused and one in flight is cut | `crates/browser-engine/src/drive/browser/takeover.rs:224` `take_over` | `browser` |
| B71 | "The browser needs you": `ask_operator` aimed at a browser raises an Ask whose only answer is Take over; while it waits the chat's acts on that browser are refused | `app/src-tauri/src/browserneedsyou.rs:295` `answered_first`, :271 `waits_said`; routed at `crates/purlis-cli/src/mcp.rs:186` | `browser` |
| B72 | A chat's browsers end when the chat closes; a pinned one outlives it (and never idles) until the person stops it, Stop all, or quit | `app/src-tauri/src/chatbrowsers.rs:3128` `forget`, :1845 `pin` | `browser` |
| B73 | No browser starts while the kill switch is thrown | `crates/host/src/hosting.rs:2673` `start_browser` → :2244 `let_through` | `browser` |
| B74 | At quit, keep the project's open browser addresses (scheme/host/port/path only, http/https, ≤ 32, ≤ 2048 bytes), none of a browser that held a login or that the person typed in; offered once at the next start, never loaded by itself | `crates/purlis-core/src/browsersatquit.rs:29` `MOST`, :32 `LONGEST`, :51 `cut`; `app/src-tauri/src/chatbrowsers.rs:1200` (filter), :6036 `kept_at_quit` | `browser` |
| B75 | Browser timeline (on this device, no chat reads it): newest 200 moments, 24 screenshots, ≤ 4 MiB each, 96 MiB a project, 16 chats; no typed text, no address of a signed-in site | `crates/purlis-core/src/browsertimeline.rs:51`, :54, :57, :61, :64 | `browser` |
| B76 | Browser proof on a PR: only screenshots the person picks (≤ 24), only after showing exactly what goes, through the person's own forge sign-in, never as a chat | `crates/purlis-core/src/browserproof.rs:201` `show`, :308 `attach`, :42 `MOST_PICKED` | `browser` |
| B77 | Engine logs kept: newest 20 across browsers; a stopping engine gets 2 s before its group is killed | `crates/purlis-core/src/browsers.rs:129` `ENGINE_LOGS_KEPT`, :98 `STOP_GRACE` | `browser` |
| B78 | DevTools (person's inspect) may call only allow-listed CDP methods, on the inspected page's origins | `crates/browser-engine/src/drive/browser/inspect/allowed.rs:29` `METHODS` | `browser` |
| B79 | The app's own webviews never navigate off the app's origin and never open a new window (not the chat browser, but the window's wall) | `app/src-tauri/src/navguard.rs:23` `stays_in_the_app`, :52 `no_new_window` | `browser` |

## Skills

| # | Decision | Where | Family |
|---|---|---|---|
| 1 | Whether the plane's skills (project/workspace/persona levels) reach chats at all | crates/purlis-core/src/skills.rs:1966 (`reach_chats`); read at crates/purlis-core/src/harness_plugin.rs:800, crates/purlis-core/src/hoststart.rs:814, crates/purlis-cli/src/mcp.rs:311, crates/purlis-core/src/sandbox.rs:1866, app/src-tauri/src/chats.rs:4658 | `skills` |
| 2 | The Skill studio flag gates every `purlis skill` verb (refused when off) | crates/purlis-cli/src/skill.rs:1106 | `skills` |
| 3 | The skill tools (`skill_list/runs/find/load`) are listed and answered only where the plane's skills reach the chat | crates/purlis-cli/src/mcp.rs:92 (list), crates/purlis-cli/src/mcp.rs:141 (call) | `skills` |
| 4 | The four skill tools are pre-allowed (no permission prompt) | crates/purlis-core/src/chattools.rs:229 (`PRE_ALLOWED` incl. skill tools at :240-243); written as allow rules at crates/purlis-core/src/harness/claude.rs:850 | `tools` |
| 5 | Route a chat's skills take, by harness: Claude Code = plugin dir, opencode = config `skills.paths`, Codex = SessionStart briefing | crates/purlis-core/src/harness.rs:517 (`Harness::skills`) | `skills` |
| 6 | A declared (ACP/other) harness gets skills only by MCP (`Route::Mcp`), and only if its declaration says it takes MCP servers; otherwise no skills | crates/purlis-core/src/skills.rs:2084 (`route_of`); not-supported listing skills.rs:3396 | `skills` |
| 7 | What a running chat picks up when skills change: MCP route everything; plugin/config/briefing nothing until restart | crates/purlis-core/src/harness.rs:539 (`skills_reload`) | `skills` |
| 8 | Levels and precedence: built-in > (name never taken); persona over workspace over project; local over shared; repo copy wins over plane copy | crates/skills/src/lib.rs:1081 (`skill_set`); rank at lib.rs:1413 | `skills` |
| 9 | Names starting `purlis-` are purlis's own: a plane skill with one is left out (never deleted), and creating/installing/renaming to one is refused | crates/skills/src/lib.rs:125 (`RESERVED_PREFIX`), :130 (`name_reserved`), left out at lib.rs:1503; refused at crates/purlis-core/src/skills/verbs.rs:309 and crates/purlis-core/src/skills/source/install.rs:262 | `skills` |
| 10 | A built-in skill's name is never turned off for a harness (switches by name would reach purlis's own), nor the index skill | crates/purlis-core/src/skills.rs:178 (`skills_off_for`) | `skills` |
| 11 | Project skill grant: reaches root chats only unless its `<folder>.toml` `grant` says `all` or named workspaces; unreadable grant = root | crates/skills/src/grant.rs:19 (enum), :45 (`reaches`), :66 (`grant_of`); applied at crates/skills/src/lib.rs:1286 (`reach`) | `skills` |
| 12 | A workspace can turn off a project skill granted to it, never turn one on that was not granted | crates/purlis-core/src/skills.rs:1828 (`switch`), :1776 (`turned_off` = `settings.skills.off`) | `skills` |
| 13 | A persona's `skills:` list brings named skills (built-in, harness, another workspace's `workspaces/<ws>:<name>`, plane) into every workspace, whatever the workspace was granted | crates/skills/src/lib.rs:1130 (comment and rule), :1180 (`persona_levels`), :1218 (`named_workspaces`); key crates/purlis-core/src/skills/persona.rs:15 | `skills` |
| 14 | Local skills (`skills.local/`) are used only while git keeps them on this machine (ignored); otherwise or if git cannot be asked, left out | crates/skills/src/lib.rs:1391 (`kept_here`); dirs lib.rs:929/932 | `skills` |
| 15 | An archived plane skill never loads and takes no name | crates/skills/src/lib.rs:1492 (Archived check) | `skills` |
| 16 | A workspace/persona skills folder made while a sandboxed chat ran is left out until the person accepts it in the Studio; a sandboxed chat whose start cannot be noted holds every later folder ("unnoted") | crates/purlis-core/src/skills/levels.rs:163 (`note_or_hold`), :459 (`accept`), :516 (`sweep`); reason crates/skills/src/lib.rs:1267 (`made_since`) | `skills` |
| 17 | A sandboxed chat is not started where this machine keeps no app state to note its levels | crates/purlis-core/src/skills/levels.rs:193 (`refusal_for_a_sandboxed_start`) | `skills` |
| 18 | Sandbox denies a chat writing every plane level's `skills/` and `skills.local/` (root, each workspace, each persona, and any made later by pattern) | crates/purlis-core/src/sandbox.rs:1332 (`skills_held`), applied at :1866 | `skills` |
| 19 | Sandbox denies a chat writing the Studio's records in state (skill-levels, drafts, tries, grades, grades-under-way, after-edit opt-in) | crates/purlis-core/src/skills.rs:80 (`KEPT_IN_STATE`), applied at crates/purlis-core/src/sandbox.rs:2513 | `skills` |
| 20 | Sandbox denies a chat writing `.claude/skills` (and `.agents`, `.codex`, `.opencode`) in a clone as planted code | crates/purlis-core/src/sandbox.rs:1109 (`PLANTED` `.claude/skills`), :1117 `.codex`, :1120 `.agents` | `skills` |
| 21 | A chat's Write/Edit/MultiEdit of a plane skill's file is refused and kept as a draft skill instead (never written) | crates/purlis-core/src/toolhooks.rs:609 (`drafting`); app side app/src-tauri/src/skilldrafts.rs:42 (`drafted`); CLI `purlis propose` crates/purlis-cli/src/brokered.rs:280 | `skills` |
| 22 | What counts as a draftable skill file: under a plane level, ≤8 folders deep, no dot-name, no `package.json`/`hooks`, no link, level must exist | crates/purlis-core/src/skills/verbs/drafts.rs:98 (`draftable`), DEEPEST :62; `SKILL_NEVER` crates/purlis-core/src/proposed.rs:228 | `skills` |
| 23 | One draft holds at most 32 files; Accept refuses a draft whose base changed since | crates/purlis-core/src/skills/verbs/drafts.rs:59 (`MOST_FILES`), :420; stale :229 | `skills` |
| 24 | Proposed changes to `.claude/skills` files (root, workspace, persona) are proposable by a chat; folders may be created only below `.claude/skills` | crates/purlis-core/src/proposed.rs:95 (`skills` closure in `proposable`, :72), :223 (`below_skills_at`) | `skills` |
| 25 | A chat never changes skills via `purlis skill`: set verbs (rm/off/on/pin/archive/mv/export several) are proposed to the person; edit/new become drafts; anything else refused | crates/purlis-cli/src/skill.rs:1143-1160 (gate), :621 (`at_the_persons_terminal`), :601 (`FROM_A_CHAT`) | `skills` |
| 26 | A change to several skills runs only at the person's interactive terminal (TTY); a detached/scripted caller proposes | crates/purlis-cli/src/skill.rs:1144 (`interactive()`), set_is_bulk :634 | `skills` |
| 27 | A change proposed from a chat names at most 500 skills | crates/purlis-core/src/skills/verbs/set/proposed.rs:32 | `limits` |
| 28 | Skill-changing window commands (install, save, delete, grant, pin, router, budget, grade, drafts…) are window-only: no link or chat can call them | crates/session-protocol/src/ui.rs:203 (`WINDOW_ONLY`, skill entries ~:782-930) | `skills` |
| 29 | `[skill_policy]` allow list: absent = every source not blocked allowed; given = only listed sources/skills | crates/purlis-core/src/skills/source/policy.rs:295 (`admitted`) | `skills` |
| 30 | `[skill_policy]` block list; block wins over allow | crates/purlis-core/src/skills/source/policy.rs:295 | `skills` |
| 31 | Minimum commit age before install/update | crates/purlis-core/src/skills/source/policy.rs:45 (`DEFAULT_MIN_AGE_DAYS`), :323 (`old_enough`) | `skills` |
| 32 | An extension source's revision cannot say its age, so it is refused while min age is on unless exempt | crates/purlis-core/src/skills/source/policy.rs:333 (made = None branch) | `skills` |
| 33 | `[skill_policy]` is read only from the shared file; one in the machine-local file refuses every install/update | crates/purlis-core/src/skills/source/policy.rs:177 | `skills` |
| 34 | An unreadable `[skill_policy]` (or unparsable file mentioning it) refuses every install and update | crates/purlis-core/src/skills/source/policy.rs:193, :203 | `skills` |
| 35 | Policy is checked at review, install, update, extension install and restore of a source-installed skill | crates/purlis-core/src/skills/source/install.rs:244; update.rs:511, :814; extension.rs:539, :559; review.rs:930; verbs/set.rs:627; verbs.rs:2209 (`hold_restored`, policy.rs:772) | `skills` |
| 36 | An org project (has `[skill_policy]`) audits every install/update/removal before it is made; refused where nothing can write the audit (only Studio or person's terminal can) | crates/purlis-core/src/skills/verbs.rs:325 (`audited`); policy.rs:730 (`NOT_AUDITED_HERE`) | `skills` |
| 37 | Session host refuses a terminal's skill-change audit on a project without controls; composes the entry itself as `human/operator` | crates/purlis-core/src/hookchannel/record.rs:90 (`audit_skill_change`); CLI side crates/purlis-cli/src/skillaudit.rs | `skills` |
| 38 | `[[skill_sources]]`: shared file sources must be `https://` with a plain host, no credential in URL; local file may also name `file:///` or an absolute folder; branch charset/length limits | crates/purlis-core/src/skills/source.rs:394 (`url_problem`), :311 (`Remote::check`), branch :433 | `skills` |
| 39 | A sign-in (vault/key) for a source is read only from the machine-local file; a shared source's own vault entry is dropped | crates/purlis-core/src/skills/source/listed.rs:112 (`sources`) | `skills` |
| 40 | A sources file with any unreadable row lists no sources at all; a local file git would carry is not read | crates/purlis-core/src/skills/source/listed.rs:57 (`listed_in`), :96 (`read`) | `skills` |
| 41 | A source named only by a committed lockfile is not contacted for updates until this machine confirms it | crates/purlis-core/src/skills/source/update.rs:116; confirm listed.rs:241 | `skills` |
| 42 | Updates are offered as a reviewed diff, never applied by themselves | crates/purlis-core/src/skills/source/update.rs:4 (module rule), `update` fn | `skills` |
| 43 | What is installed is what was reviewed: install refetches and refuses if any digest/pin moved | crates/purlis-core/src/skills/source/install.rs:237 | `skills` |
| 44 | A name clash at the landing level stops the review until skipped or renamed (`<source>-<name>`) | crates/purlis-core/src/skills/source/review.rs:44 (`Clash`); install.rs:271 (`free_at`) | `skills` |
| 45 | Fetch limits: one skill ≤16 MiB, reads ≤64 MiB, one fetch ≤256 MiB; hardened git (no hooks, no submodules, fsck, depth 1) | crates/purlis-core/src/skills/source/fetch.rs:49, :52, :55 | `limits` |
| 46 | Source tree limits: file ≤1 MiB, ≤1000 files per skill, skill depth ≤4, ≤50,000 entries, ≤200 skills per source, path ≤1024 bytes | crates/skills/src/tree.rs:16, :19, :22, :99, :102, :106 | `limits` |
| 47 | Install review marks (not refuses) scripts, network hosts, `!` lines, elevation (`sudo/doas/pkexec`), out-of-workspace writes, invisible chars, binaries | crates/skills/src/review.rs:284 (`NETWORK_WORDS`), :289 (`ELEVATING`), :292 (`RUNNERS`), :297 (`WRITERS`) | `skills` |
| 48 | An extension skill source is asked only where this machine approved the extension and the project has it on; its answer held to 64 files / 256 KiB per skill, revision ≤128 bytes | crates/purlis-core/src/extension/studio.rs:887-888, :910 (`MOST_REVISION_BYTES`), crates/purlis-core/src/skills/studio.rs:20 (gate doc) | `skills` |
| 49 | Publish is refused from a chat or without a person at a TTY; no flag skips its question | crates/purlis-cli/src/skill.rs:3160 (`publish`), :3264 (`may_publish`); window-only app/src-tauri/src/skillpublish.rs | `skills` |
| 50 | Export/publish stops: credential-shaped content (both), personal data/home path/machine-local skill (publish); leaves behind links, secret-named files, git-ignored files; no override | crates/purlis-core/src/skills/source/outgoing.rs:82 (`Stop`), :40 (`MOST_FILES` 2,000) | `skills` |
| 51 | Publish goes to the source if the forge says push allowed (or does not say), else to the person's own fork (made only after yes); never forced, never to an org | crates/purlis-core/src/skills/source/publish.rs:16 (rule), :869 (fork wait 60 s) | `skills` |
| 52 | Harness/repo skills the project turns off per harness (`[harness_skills.<harness>]`), local > workspace > shared | crates/purlis-core/src/harness_plugin.rs:43 (`SKILLS_TABLE`); handed as Claude `skillOverrides` crates/purlis-core/src/harness/claude.rs:769, Codex `skills.config` crates/purlis-core/src/harness/codex.rs:103, opencode `permission.skill` deny crates/purlis-core/src/opencode.rs:341 | `skills` |
| 53 | A plugin's skill cannot be turned off alone in Claude Code (goes with its plugin), so purlis does not write it | crates/purlis-core/src/harness/claude.rs:758 (comment), `skill_named_alone` crates/purlis-core/src/harness_plugin.rs:184 | `skills` |
| 54 | A skill handed by purlis never carries `allowed-tools` or `hooks` (they widen the chat); `agent` kept only for `context: fork` with a harness agent; respelled widening keys refused | crates/skills/src/handed.rs:679 (`emit`), :706 (`agent_kept`), :410 (respelled); fields crates/skills/src/compat.rs:145 (`widens`) | `tools` |
| 55 | A manual-only skill (`disable-model-invocation`) is handed per harness: Claude reads it; opencode gets it without a description | crates/purlis-core/src/harness/opencode.rs:190 (`skill_frontmatter`); claude/codex adapters' `skill_frontmatter` | `skills` |
| 56 | `skill_load` refuses a manual-only skill unless `person_asked=true` (model's word, not a security boundary) | crates/purlis-core/src/skilltools.rs:332 | `skills` |
| 57 | `skill_load` refuses a narrowing skill the harness was handed and can start itself, pointing to the harness's own start; elsewhere loads with "Narrowing not applied" | crates/purlis-core/src/skilltools.rs:338, `narrowed_start` :194 | `skills` |
| 58 | Skill tools answer only over the chat's own set; never by path; `skill_find` ≤10 results, `skill_runs` ≤20 runs; load names ≤200 other files, depth ≤4 | crates/purlis-core/src/skilltools.rs:240 (`call`), :65 (`MOST_RUNS`); crates/skills/src/load.rs:34, :37, :40 | `skills` |
| 59 | SKILL.md read limit 256 KiB; frontmatter nesting ≤64; `<folder>.toml` ≤64 KiB; ≤2000 names read per folder | crates/skills/src/handed.rs:290, :287; crates/skills/src/beside.rs:12; crates/skills/src/held.rs:95 | `limits` |
| 60 | Skill name ≤64 chars, description ≤1024 (open format); lint blocks a save on a non-whole skill | crates/skills/src/change.rs:12, :15; crates/skills/src/lint.rs:35 (`Severity::Blocks`) | `limits` |
| 61 | Skill listing budget per harness (skills / chars / per-description) | crates/purlis-core/src/harness.rs:33 (`SKILL_BUDGET` 30/6,000); claude crates/purlis-core/src/harness/claude.rs:275 (60/15,000/1,536); codex crates/purlis-core/src/harness/codex.rs:309 (8,000 chars/1,024, `DEFAULT_CHARS` :58); opencode crates/purlis-core/src/harness/opencode.rs:178 (8,000) | `skills` |
| 62 | Budget layering: project then workspace, number by number; a value past a harness limit is warned, never refused; warn at 80% | crates/purlis-core/src/skills/budget_setting.rs:135 (`budget_in`); crates/skills/src/budget.rs:23 | `skills` |
| 63 | Hard listing caps measured from harnesses: Codex keeps 10,000 bytes of hook output; Claude Code drops descriptions past 100,000; built-ins' descriptions ≤5,000; Codex leaves ≥2,800 for plane skills | crates/purlis-core/src/harness/codex.rs:16; crates/purlis-core/src/harness/claude.rs:1048; crates/purlis-core/src/skills.rs:210, :215 | `skills` |
| 64 | Built-in router: above budget hides every non-pinned plane skill and shows the index skill; never hides built-ins, harness/repo, pinned, manual-only | crates/skills/src/router.rs:197 (`routed`); index crates/skills/src/router.rs:30 | `skills` |
| 65 | Which router a chat uses: persona `skill-router:` > workspace `settings.skills.router` > built-in; a bad value = built-in | crates/purlis-core/src/skills/router.rs:61 (`chosen`) | `skills` |
| 66 | Extension router gets 100 ms (incl. its confined start), sees only names/descriptions/pins/budget; an answer that hides a pin, hides a non-hideable skill or stays over budget is replaced by the built-in router | crates/purlis-core/src/extension/studio.rs:1068 (`ROUTER_DEADLINE`); crates/skills/src/router.rs:285 (`routed_by`) | `limits` |
| 67 | A router is handed a chat's prompts only if its approval said it reads prompts (and the person has not turned that off) | crates/purlis-core/src/extension/studio.rs:1099; crates/purlis-core/src/skills/router.rs:393 (`on_prompt`); env `PURLIS_PROMPT_ROUTER` skills.rs:457 | `skills` |
| 68 | Pins: workspace `settings.skills.pinned` + persona `pinned-skills:`; ≤64 pins, each a valid name; past that the chat has no pins | crates/purlis-core/src/skills/routing.rs:20 (`MOST_PINS`), :31 (`pins_of`); persona key crates/purlis-core/src/skills/persona.rs:19 | `skills` |
| 69 | Use skill… types a skill's start into the chat, never sends it; refuses a skill not in the chat's set | crates/purlis-core/src/skills.rs:1364 (`typed`); app/src-tauri/src/useskill.rs | `skills` |
| 70 | Try it: scratch chat in a throwaway piece, armed with built-in skills and the one draft alone, every harness plugin and person's own skill off; piece removed on close | crates/purlis-core/src/skills/verbs/drafts.rs:685 (`try_out`), :1374 (`tried`), :1507 (`alone_on`) | `skills` |
| 71 | A grade's chat must start in the full sandbox, keeping none of the person's Allows, on a profile that keeps permission prompts; outcome-eval chat with a shell reaches no host and writes only its folder | crates/purlis-core/src/skills/grades.rs:2187 (`holding_no_kept_grants`/`holding_to_its_folder`), :2282 (`chat_refused`) | `fixed` |
| 72 | Which harnesses can grade: Claude Code yes; Codex and opencode refused | crates/purlis-core/src/harness/claude.rs:417; crates/purlis-core/src/harness/codex.rs:397; crates/purlis-core/src/harness/opencode.rs:242 | `fixed` |
| 73 | Claude grade refused when profile bypasses prompts, uses a non-asking mode, or loads its own `--plugin-dir`; outcome evals refused when sign-in is in Claude settings | crates/purlis-core/src/harness/claude.rs:493 (`grade_refused`), :478 (`outcome_refused`) | `fixed` |
| 74 | A grade's chats have only purlis's plugin on; model must be one the adapter offers | crates/purlis-core/src/harness_plugin.rs:717 (`only_purlis_on`); crates/purlis-core/src/skills/grades.rs:2199, :2225 (`models_for`) | `fixed` |
| 75 | A grade's chat is served only the read-only (pre-allowed) MCP tools | crates/purlis-cli/src/mcp.rs:93 (list), :109 (call) | `fixed` |
| 76 | Grade token cap (and derived dollar limit) | crates/purlis-core/src/skills/grades/cap.rs:26 (`DEFAULT`), :29-30 (`LEAST`/`MOST`) | `fixed` |
| 77 | A grade's chat runs at most 5 min; must reach session start within 120 s; one grade at a time per project | crates/purlis-core/src/skills/grades.rs:53 and app/src-tauri/src/skillgrade.rs:56 (`CHAT_TIME`); skillgrade.rs:61, :52 (`A_GRADE_RUNS`) | `fixed` |
| 78 | Grade shape: 3 runs per test, trigger threshold 0.5, stops after 2 not-started in a row; fixtures ≤100 trigger tests, ≤100 outcome evals, prompt ≤8 KiB, ≤32 expectations | crates/skills/src/grade.rs:40, :43, :48; crates/skills/src/grade/fixtures.rs:30, :33, :37, :40 | `fixed` |
| 79 | Grade after each edit: off per skill until the person turns it on from a plan; runs only within the chats and cost accepted | crates/purlis-core/src/skills/grades/after_edit.rs:442 (`next`), FILE :30 | `skills` |
| 80 | Grades never go in a skill's folder; eval record committed under `personas/_grades/` | crates/purlis-core/src/skills/grades.rs:32 (`DIR_NAME`) | `skills` |
| 81 | Deleted skills kept 30 days (expiry 31), at most 30 listed; history ≤100 versions | crates/purlis-core/src/skills/verbs.rs:1485, :1488, :1970; crates/purlis-core/src/skills/history.rs:37 | `skills` |
| 82 | Skills unused for 60 days are flagged (not removed) | crates/purlis-core/src/skills/lifecycle.rs:22 (`UNUSED_DAYS`) | `skills` |
| 83 | Once a project moved to `skills/` (project-skills fix), no `.claude/skills` is copied into a clone; before that the plane's `.claude/skills` is mirrored | crates/purlis-core/src/guest.rs:105 (`RETIRED_MIRROR`), :98 | `skills` |
| 84 | Studio offers well-known sources from a committed list; an offer adds/installs nothing | crates/purlis-core/src/skills/first_open.rs:30 (`OFFERED`, offered.toml) | `skills` |

## The harness permission mode

| # | Decision | Where | Family |
|---|---|---|---|
| 1 | purlis does not set the permission mode of a person-started chat. The mode is whatever the profile's `command` and the person's own harness settings say. The person picks bypass, auto, etc. by putting flags in a profile. | profile command: `crates/purlis-core/src/settings/harness_profiles.rs:8`; read `crates/purlis-core/src/attendance.rs:129` (`ClaudeCodeMode::set_in`) | `mode` |
| 2 | A new or changed profile command needs the person's approval before its first run. An unreadable approval record means ask again. | `crates/purlis-core/src/profiletrust.rs:283` (`approval_needed`), `:26` (`RECORD`) | `trust` |
| 3 | Flags that count as "prompts off": `--dangerously-skip-permissions`, `--dangerously-bypass-approvals-and-sandbox`, `--yolo`, `--full-auto`, `--permission-mode bypassPermissions`, Codex `-a/--ask-for-approval never`, `-c/--config approval_policy=never`. Recognition only; a wrapper script is not seen. | `crates/purlis-core/src/dispatchunattended.rs:241` (`BYPASS_FLAGS`), `:252` (`MODE_FLAGS`), `:278` (`bypass_in`) | `mode` |
| 4 | Only Claude Code's mode is read, and only from its armed hook (the hook carrying the armed key). An unproven line's mode reads as "asks". Codex reports bypass on every line and is not read. | `crates/purlis-core/src/attendance.rs:181-195`; `crates/purlis-core/src/dispatchmode.rs:119` (`mode_unproven`); `crates/purlis-core/src/harness/claude.rs:409` (`reports_a_mode_purlis_can_check`) | `mode` |
| 5 | A Claude chat reporting a mode purlis didn't set flips it to unattended and raises a standing Notice (ModeDiffers), never an ask. | `crates/purlis-core/src/attendance.rs:353` (`mode_differs`); `app/src/ModeDiffersNotice.tsx` | `mode` |
| 6 | A task starts in its asker's mode, capped at auto: default → default; auto → auto; bypass → auto. Other modes (dontAsk, acceptEdits, plan) start the task at the default. Bypass never travels. | `crates/purlis-core/src/dispatchmode.rs:93` (`for_a_task`), `:72` (`CLAUDE_CODE_AUTO`) | `mode` |
| 7 | A task at auto gets `--permission-mode auto` added, unless the profile command names its own `--permission-mode`. | `crates/purlis-core/src/harness/claude.rs:449` (`task_mode_words`) | `mode` |
| 8 | A task does not start in auto if the person's Claude settings (user, remote-settings, or managed policy files) hold `autoMode.soft_deny`/`hard_deny` rules other than `$defaults`. It starts asking and says why. | `crates/purlis-core/src/harness/claude.rs:1304` (`settings_keep_auto_from_a_task`), `:1328`, `:1290` (`AUTO_KEPT`); `crates/purlis-core/src/dispatchmode.rs:140` (`task_start`) | `mode` |
| 9 | No chat one chat starts for another (task, persona chat, handoff) runs on a prompts-off profile, whoever named it: the asker's profile, `--profile`, or the persona's `profile:`. | `crates/purlis-core/src/dispatchunattended.rs:331` (`bypass_refusal`), `:381`; `app/src-tauri/src/handoff.rs:4136` | `mode` |
| 10 | A dispatched chat is not restarted on a profile that has since turned prompts off. | `crates/purlis-core/src/dispatchprofiles.rs:202` (`started_again_refusal`) | `mode` |
| 11 | A skill grade's chat: only `default` or `plan` mode. Refused if the profile turns prompts off, names another mode, loads its own `--plugin-dir`, or the person's Claude settings set `defaultMode: bypassPermissions` or cannot be read. `permissions.defaultMode=default` is pinned. Sub-agent tools (`Task`, `Agent`) are denied. | `crates/purlis-core/src/harness/claude.rs:1177` (`GRADED_MODES`), `:493` (`grade_refused`), `:960` (pinned), `:1017` | `fixed` |
| 12 | Allow rules purlis adds to every app-started Claude chat's `--settings` (session only, merged): Smart close (`purlis session record`), dispatch / dispatch-task / handoff commands, read-only `purlis` commands, `worktree commit`, own-scope commands (todo, worktree update/push/done/abandon/list/remove/prune), and rules naming its own persona and workspace. | `crates/purlis-core/src/harness/claude.rs:820-841`; lists `:1420` (`SMART_CLOSE_ALLOW`), `:1433` (`READ_ALLOW`), `:1475` (`OWN_SCOPE_ALLOW`), `:1507` (`own_scope_rules`), `:1560` (`WORKTREE_COMMIT_ALLOW`), `:1577` (`DISPATCH_ALLOW`), `:1600` (`DISPATCH_TASK_ALLOW`), `:1629` (`HANDOFF_ALLOW`) | `tools` |
| 13 | purlis MCP tools allowed without a prompt: 18 read tools (`PRE_ALLOWED`), 11 own-scope tools (todo_add, memory_add, ask_operator, secret_ask…), every dispatch tool, and browser tools except `browser_upload`/`browser_save_download`. | `crates/purlis-core/src/chattools.rs:229`, `:264`, `:614` (`run_on_the_dispatch_grant`); `crates/purlis-core/src/browsertools.rs:83`, `:94` (`ASKED_EACH_TIME`); assembled `crates/purlis-core/src/harness/claude.rs:846-872` | `tools` |
| 14 | Codex: purlis's MCP server tools get `approval_mode="approve"` for each dispatch tool the operator set nothing for. The operator's own per-tool or default approvals are carried over and win. MCP tool timeout is 3600 s. | `crates/purlis-core/src/chattools.rs:415` (`CodexApprovals`), `:515`, `:322` (`CODEX_TOOL_TIMEOUT_SECS`); armed `crates/purlis-core/src/harness/codex.rs:230` | `tools` |
| 15 | opencode: `permission` in `OPENCODE_CONFIG_CONTENT` allows each dispatch tool unless an operator rule names it, and denies skills the project turned off. No bash permissions are written. | `crates/purlis-core/src/opencode.rs:373-391`, `:385` (`leaves_unasked`) | `tools` |
| 16 | A sandboxed Claude chat with purlis's wrap instead of Claude's own sandbox gets `allow: ["Bash"]`, since Claude's `autoAllowBashIfSandboxed` no longer covers it. | `crates/purlis-core/src/harness/claude.rs:878` | `tools` |
| 17 | When the sandbox is on, Claude's `sandbox` object and `permissions.deny` (Read/Edit deny rules matching the sandbox) are written into the session settings. | `crates/purlis-core/src/harness/claude.rs:890-897` | `sandbox` |
| 18 | A persona's `disallowed-tools:` become `permissions.deny` (outranks every allow). | `crates/purlis-core/src/harness/claude.rs:1010` (`denied_tools`), `:900-910` | `tools` |
| 19 | Outcome-eval chats with no shell: Bash allow rules stripped, `Bash` added to `ask`, `autoAllowBashIfSandboxed=false`. Judge chats are denied 12 tools. | `crates/purlis-core/src/harness/claude.rs:883`, `:894`, `:948`, `:1103` (`JUDGE_DENIED`) | `fixed` |
| 20 | A sandboxed Claude chat loads its project's settings files only if every key is in `KEPT` (`$schema`, `permissions`, `enabledPlugins`, `model`, `outputStyle`, `theme`) and `permissions` holds only deny/ask. Otherwise purlis carries the project's deny/ask rules itself. A change re-screens and restarts the chat. | `crates/purlis-core/src/harness/claude/project.rs:31` (`KEPT`), `:41` (`KEPT_RULES`), `:44`; carried `crates/purlis-core/src/harness/claude.rs:916`; watch `app/src-tauri/src/projectsettings.rs:4-17` | `mode` |
| 21 | Rules naming `mcp__charter__<tool>` (ask/deny only) get a `mcp__purlis__` twin. | `crates/purlis-core/src/harness/claude.rs:935`; `crates/purlis-core/src/scaffold/settings.rs:631` | `tools` |
| 22 | A sandboxed Codex chat: `--sandbox danger-full-access` (Codex's own sandbox off inside purlis's wrap). `approval_policy` is granular (`sandbox_approval=false`, `request_permissions=false`, `skill_approval=false`, `rules=true`, `mcp_elicitations=true`); `approvals_reviewer=user`; `web_search=disabled`; `browser_use`/`computer_use`/`in_app_browser` disabled. | `crates/purlis-core/src/sandbox/codex.rs:774` (`flags`), `:783`, `:792-794`, `:803`; used `crates/purlis-core/src/harness/codex.rs:502` | `sandbox` |
| 23 | A sandboxed Codex chat is refused if its words carry a loosening flag: `-s/--sandbox`, `--dangerously-bypass…`, `--yolo`, `--add-dir`, `-C/--cd`, `--approve-for-me`, `--not-so-yolo`, `--search`, `--enable/--disable`, `-a/--ask-for-approval` with any value but `never`, or a `-c` key outside the 12 model/display keys. | `crates/purlis-core/src/sandbox/codex.rs:813` (`LOOSENING`), `:830` (`APPROVAL`), `:840` (`CONFIG_ALLOWED`), `:946` (`loosened_by`); refusal `crates/purlis-core/src/harness/codex.rs:441` | `sandbox` |
| 24 | An unsandboxed Codex chat keeps Codex's own approval and sandbox settings. purlis adds only hooks, the permission hook and its MCP server. | `crates/purlis-core/src/harness/codex.rs:209-211`, `:212` (`arm_under`) | `mode` |
| 25 | opencode: an external plugin is loaded per session. `--pure` / `OPENCODE_PURE=1` (which would disarm the guard) refuses the chat. | `crates/purlis-core/src/opencode.rs:680` (`disarmed_by`), `:93`, `:96` | `guards.shape` |
| 26 | ACP: `initialize` offers no `fs` and no `terminal`, and elicitation in form mode only. A sandboxed project never runs a chat over ACP. The agent's own permission options are shown, minus allow_always. | `crates/purlis-core/src/acp.rs:15-18`, `:1488`; `crates/purlis-core/src/acp/launch.rs:63`, `:88`; `crates/purlis-core/src/harness/asked.rs:262` | `sandbox` |
| 27 | ACP handshake patience is 60 s. An agent that needs a login is not started (purlis never calls `authenticate`). | `crates/purlis-core/src/acp.rs:459` (`HANDSHAKE_PATIENCE`), `:22-24` | `trust` |
| 28 | Which harness can take a command rule: Claude → `.claude/settings.json` (or `.local` with `--local`); opencode → `opencode.json permission.bash` (no `--local`); Codex → none ("purlis's own guard applies"). | `crates/purlis-core/src/harness/claude.rs:373`, `crates/purlis-core/src/harness/opencode.rs:198`, `crates/purlis-core/src/harness/codex.rs:370` | `tools` |
| 29 | The persona tool gate auto-allows (answers `allow`, never denies) a Bash command the active persona's `tools:` declares. It declines for shell metacharacters, interpreters/wrappers, destructive subcommands (`DANGEROUS`), and control-surface paths. Its ceiling is frozen at SessionStart and bound to the plane-trust approval. | `crates/purlis-core/src/personagate.rs:850` (`decide`), `:79` (`DANGEROUS`), `:117` (`INTERPRETERS`), `:760` (`snapshot`), `:901` | `tools` |
| 30 | A grade's settings: plugins pinned (bundled on; Python charter plugin off), regardless of project files. | `crates/purlis-core/src/harness/claude.rs:744-751` | `fixed` |

## Admin locks

| # | Decision | Where | Family |
|---|---|---|---|
| L1 | Where policy is read from: one fixed path, nothing in a project or the environment moves it | crates/purlis-core/src/sandbox/policy.rs:160 (`MACHINE_FILE`), :267 (`Locks::of`), :317 (`of_this_machine`) | `fixed` |
| L2 | Policy file trusted only if regular file, not a link, root-owned, in a root-owned folder, not group/other-writable; ACLs not read | crates/purlis-core/src/sandbox/policy.rs:1119 (`judge`), :1140 (`judge_folder`), :1160 (`read_trusted`, unix) | `fixed` |
| L3 | Policy file at most 64 KiB, `owner` clipped to 120 chars, refusal sentence to 240 | crates/purlis-core/src/sandbox/policy.rs:163, :166, :169 | `fixed` |
| L4 | No policy file read off Unix (Windows: no policy at all) | crates/purlis-core/src/sandbox/policy.rs:1216 (`#[cfg(not(unix))] read_trusted` returns None) | `fixed` |
| L5 | A refused file (untrusted, not JSON, unknown key, bad value) refuses closed: every lock set except `presets`; dispatch off | crates/purlis-core/src/sandbox/policy.rs:344 (`refused`); unknown keys :869, :889, :838 (`KEYS`, 16 sandbox keys), :1041 (`DISPATCH_LOCKS`); crates/purlis-core/src/dispatchlimits.rs:1385 (`ceiling_when_refused`) | `fixed` |
| L6 | App re-reads the policy every 5 s and re-lists chats running under an older sandbox | app/src-tauri/src/sandboxing.rs:3634 (`POLICY_READ_EVERY`), :3670 (`watch_policy`) | `fixed` |
| L7 | `sandbox.presets`: which Internet access presets a project may turn on; others off | parse crates/purlis-core/src/sandbox/policy.rs:915; decide :439 (`presets`), :448 (`allows_preset`); read at crates/purlis-core/src/sandbox.rs:4452, :548 (forge preset), crates/purlis-core/src/sandbox/local.rs:891, :902, crates/purlis-core/src/gitbroker.rs:947, app/src-tauri/src/network.rs:383, app/src-tauri/src/sandboxing.rs:408-410 (Settings) | `network` |
| L8 | `sandbox.hosts`: the only hosts any level (project, its forges, you, persona, a block's Allow) may add | crates/purlis-core/src/sandbox/policy.rs:460 (`allowed_hosts`), :566 (`refuses`); read at crates/purlis-core/src/sandbox/hosts.rs:735, crates/purlis-core/src/sandbox.rs:536, :3451, crates/purlis-core/src/settings/hosts.rs:182, :305 (refused on save, `locked_out` :297, called from crates/purlis-core/src/settings.rs:1006-1008), app/src-tauri/src/sandboxing.rs:416 | `network` |
| L9 | `sandbox.personal-hosts` false: your own hosts and a block's Allow for chat/you reach no chat | crates/purlis-core/src/sandbox/policy.rs:465, :574 (in `refuses`); app/src-tauri/src/hooks.rs:1537; app/src-tauri/src/sandboxing.rs:418 | `network` |
| L10 | `sandbox.persona-hosts` false: a persona's own hosts reach no chat | crates/purlis-core/src/sandbox/policy.rs:470, :571; app/src-tauri/src/sandboxing.rs:419, :548, :868 | `network` |
| L11 | `sandbox.persona-tools` false: no persona tool shim given to a chat; project file save adding one refused; login/browser tools refused | crates/purlis-core/src/sandbox/policy.rs:475, :481 (`refuses_tool`); read at crates/purlis-core/src/sandbox/tools.rs:1414, :1493, crates/purlis-core/src/sandbox/persona.rs:415, crates/purlis-core/src/settings/hosts.rs:341 (save), app/src-tauri/src/loginasks.rs:726, :770, :1043, app/src-tauri/src/chatbrowsers.rs:3819, :4829 | `tools` |
| L12 | `sandbox.opt-out` false: requires the sandbox in every project; no chat starts unsandboxed; with no backend no harness chat starts; project without `[sandbox]` runs with default presets | crates/purlis-core/src/sandbox/policy.rs:493 (`forbids_opt_out`), :500, :826 (`opt_out_refused`); read at crates/purlis-core/src/sandbox.rs:785-790 (`Said::in_force`), :5108, :6365, :6422, :8283, :8300, crates/purlis-core/src/start.rs:564, crates/purlis-core/src/sandbox/team.rs:609, :617, crates/purlis-core/src/sandbox/protection.rs:168, app/src-tauri/src/chats.rs:4450, app/src-tauri/src/hooks.rs:1514, :1642, app/src-tauri/src/sandboxing.rs:102, :420-421 | `sandbox` |
| L13 | `sandbox.write-grants` false: no folder granted to a chat (block Allow or Settings); kept ones held | crates/purlis-core/src/sandbox/policy.rs:517, :699; read at crates/purlis-core/src/sandbox.rs:3555, crates/purlis-core/src/sandbox/grant.rs:862, app/src-tauri/src/hooks.rs:1619, :1637, app/src-tauri/src/sandboxing.rs:422, :3587 | `sandbox` |
| L14 | `sandbox.vault-grants` false: a persona's chats use only vaults the registry tags for it; Allow removed, earlier grants open nothing | crates/purlis-core/src/sandbox/policy.rs:784, :790; read at crates/purlis-core/src/secretfill.rs:64, crates/purlis-core/src/secrets/brokered.rs:1405, :1817, :1831, :1856, :1942, :2035, :3605, app/src-tauri/src/vaultroute.rs:372, :489, :975, app/src-tauri/src/vaults.rs:368, app/src-tauri/src/secretasks.rs:347, :388, :759, app/src-tauri/src/asking.rs:3292, app/src-tauri/src/loginasks.rs:729, :871, app/src-tauri/src/secretfills.rs:1152, app/src-tauri/src/sandboxing.rs:2791 | `vaults` |
| L15 | `sandbox.live-asks` false: connection to an unlisted host refused at once instead of held while the person is asked | crates/purlis-core/src/sandbox/policy.rs:523, :529; read at crates/purlis-core/src/sandbox/hosted.rs:258, :275, app/src-tauri/src/chats.rs:9692, app/src-tauri/src/hooks.rs:1413 | `asks` |
| L16 | `sandbox.socket-grants` false: no local socket, system service, loopback port or listen (server) grant; kept ones held | crates/purlis-core/src/sandbox/policy.rs:709, :714, :726 (ports), :737 (listen); read at crates/purlis-core/src/sandbox.rs:4056, :4090, crates/purlis-core/src/sandbox/local.rs:906, crates/purlis-core/src/sandbox/hosted.rs:257, app/src-tauri/src/hooks.rs:1004, :1561, :1596, :1597, app/src-tauri/src/asking.rs:1811 | `ports` |
| L17 | `sandbox.socket-grants` false also turns certificate checks off whatever `[sandbox].certificate-checks` says | crates/purlis-core/src/sandbox.rs:368 (`CertificateChecks::of`), :4113 | `ports` |
| L18 | `sandbox.proposed-changes` "off": a chat's edit of purlis's own config (manifest, persona `mcp.json`/`persona.md`, `.claude/skills`) refused, never shown to apply | crates/purlis-core/src/sandbox/policy.rs:749; parse :946; read at crates/purlis-core/src/toolhooks.rs:649, crates/purlis-cli/src/brokered.rs:274, app/src-tauri/src/proposals.rs:403, :478, :563, :983, :1262, :1309, app/src-tauri/src/skilldrafts.rs:49, app/src-tauri/src/loginasks.rs:889 | `guards.plane` |
| L19 | `sandbox.undeclared-secret-asks` false: Secret ask for a key the persona does not declare is declined by `policy`, person never asked | crates/purlis-core/src/sandbox/policy.rs:760; read at crates/purlis-core/src/secrets/ask.rs:617, crates/purlis-core/src/secrets/brokered.rs:1034, app/src-tauri/src/sandboxing.rs:423 | `vaults` |
| L20 | `sandbox.run-grants` false: `secret exec` of an undeclared program raises no Allow, uses no earlier grant (persona-tools false has same effect) | crates/purlis-core/src/sandbox/policy.rs:765, :772 (`refuses_run_grant`); read at crates/purlis-core/src/secrets/brokered.rs:1021, app/src-tauri/src/asking.rs:3290, app/src-tauri/src/secretasks.rs:374, app/src-tauri/src/sandboxing.rs:424 | `vaults` |
| L21 | `sandbox.allow-scopes`: which scopes (once/chat/you/machine/project) an Allow may keep at; others removed from Notices and refused; kept grants at removed scopes held at every start | crates/purlis-core/src/sandbox/policy.rs:625 (`keeps_scope`), :633, :654 (`holds_kept`), :671, :688; read at crates/purlis-core/src/sandbox.rs:3532-3698, :4273, :4297, :4465, crates/purlis-core/src/sandbox/local.rs:1853, :1924, crates/purlis-core/src/sandbox/browser_sites.rs:88, app/src-tauri/src/hooks.rs:1345, :1434, :1574, :1605, :1630, app/src-tauri/src/loginasks.rs:890, :1114, app/src-tauri/src/sandboxing.rs:2857, :2929-2970, app/src-tauri/src/secretfills.rs:650 | `asks` |
| L22 | A Machine-scope Allow only for a socket or service, never a host or folder | crates/purlis-core/src/sandbox/policy.rs:602 (in `refuses_grant`) | `asks` |
| L23 | `sandbox.never-hosts`: hosts no level may add, no Allow may keep, never asked about | crates/purlis-core/src/sandbox/policy.rs:540 (`never_allows`), :560; read at crates/purlis-core/src/sandbox.rs:4357, :4557 (reach) and via `refuses`/`refuses_grant` | `network` |
| L24 | `sandbox.relayed-internal`: internal addresses a desktop-planned chat on a runner may reach; absent = none; metadata never | crates/purlis-core/src/sandbox/policy.rs:549, parse :1009 (`relayed_internal`); read at crates/purlis-core/src/sandbox/internal.rs:125 | `network` |
| L25 | `dispatch.<limit>` ceilings (running-per-chat, live-per-lineage, depth ≤8, messages-per-minute, may-dispatch, may-run-at-once, tokens-per-session, minutes-per-task): no level sets more; 0 turns that off | crates/purlis-core/src/sandbox/policy.rs:506 (`dispatch_ceiling`); crates/purlis-core/src/dispatchlimits.rs:1349 (`ceiling`), :114 (`DEEPEST` 8), :117 (`MOST` 10,000), :120 (`MOST_TOKENS`); read at crates/purlis-core/src/dispatchlimits.rs:1515, app/src-tauri/src/overlimit.rs:363-364, :404, :603, :615, app/src-tauri/src/dispatchlimits.rs:179 | `fixed` |
| L26 | `dispatch.allow` false: no chat dispatches to another, its own persona included | crates/purlis-core/src/sandbox/policy.rs:800, :812 (`dispatch_refused`); read at crates/purlis-core/src/dispatchgrant.rs:448, :451, :457, crates/purlis-core/src/dispatchdecision.rs:485, crates/purlis-core/src/dispatchwants.rs:358, :379, app/src-tauri/src/handoff.rs:4412, :4489, app/src-tauri/src/dispatchgrants.rs:1010, :3533, :4003 | `dispatch` |
| L27 | `dispatch.locked`: persona pairs (from→to) no chat may dispatch across; no grant covers them | crates/purlis-core/src/sandbox/policy.rs:805, :812; parse :1068; read as L26 plus app/src-tauri/src/dispatchgrants.rs:3772, :4010 | `dispatch` |
| L28 | Outcome eval's shell: hard-set lock bundle (no hosts, presets, persona hosts/tools, write/vault/socket/run grants, live asks, scopes; own temp folder only) | crates/purlis-core/src/sandbox/policy.rs:274-291 (in `Locks::of`), :1243 (`holding_to_its_folder`); used at crates/purlis-core/src/skills/grades.rs:2189 | `fixed` |
| L29 | Grade's chat keeps no Allow of any scope | crates/purlis-core/src/sandbox/policy.rs:306-310, :1259 (`holding_no_kept_grants`); used crates/purlis-core/src/skills/grades.rs:2187 | `fixed` |
| L30 | Pool setup's Get help chat reaches no cluster: no hosts, personal/persona hosts, sockets, live asks, scopes, relayed internal | crates/purlis-core/src/sandbox/policy.rs:293-304, :1276 (`holding_from_clusters`); used crates/purlis-core/src/poolsetup/help.rs:36 | `runners` |
| L31 | Every chat denied writing the policy folder `/etc/purlis` | crates/purlis-core/src/sandbox/policy.rs:1221 (`machine_folder`), consumed by `Denied::of` (HumanPowers) | `fixed` |
| L32 | Settings shows every locked value as "Locked by policy" / "On, required by policy" with owner; no widening control | crates/purlis-core/src/sandbox/policy.rs:403 (`led_by`), :418, :424; app/src-tauri/src/sandboxing.rs:405-425 | `fixed` |

## Other


### Other (A)

| # | Decision | Where | Family |
|---|---|---|---|
| 1 | Which settings layers exist, and which one wins: Default < Shared (`charter.toml`) < Workspace (`workspace.json` `settings`) < Local (`charter.local.toml`), key by key | crates/purlis-core/src/settings.rs:54 (`Source`); crates/purlis-core/src/extension/project.rs:11-23 (order doc), :419 (`resolve`) | `trust` |
| 2 | A `charter.local.toml` that git would commit (tracked or not ignored) is read as no layer at all | crates/purlis-core/src/settings.rs:195 (`layer_text`), :219 (`local_layer_holding`) | `trust` |
| 3 | Which tables a workspace's `workspace.json` `settings` may hold | crates/purlis-core/src/settings/workspace.rs:42 (`READ`) | `trust` |
| 4 | A workspace's runners `leave_out` list: at most 64 entries of at most 256 chars | crates/purlis-core/src/settings/workspace.rs:80, :83 | `limits` |
| 5 | Settings-tab save of either file is refused for whatever the next read of that file would refuse (parse/schema, doctor rows, profiles loader, `[extensions]`, `[harness_skills]`, `[theme]`, `[sandbox]`, `[dispatch]`, `[skill_grades]`, `[task_screens]`, `[skills]`, `[plane]`/`[repos]`) — only new refusals, not ones the file already had | crates/purlis-core/src/settings.rs:459 (`read_refusals`), :468-497, :995 (`refused_of`) | `consents` |
| 6 | A save is refused if the file holds a secret-shaped value (either file) | crates/purlis-core/src/settings.rs:501 (`writer_refusals`), :659 (`secret_refusal`) | `guards.credential` |
| 7 | A Local-file save is refused if git would commit `charter.local.toml` | crates/purlis-core/src/settings.rs:503-509 | `guards.save` |
| 8 | A save naming a built-in skill in `[harness_skills]` (turning it off) is refused | crates/purlis-core/src/settings.rs:510-521 | `consents` |
| 9 | `[chat_env]` in the committed file is refused (it is read from Local only) | crates/purlis-core/src/settings.rs:583 (`chat_env_is_local`) | `trust` |
| 10 | `[persona] default` naming no persona is read as no default and refused on save | crates/purlis-core/src/settings.rs:601 (`names_nothing`) | `consents` |
| 11 | A save of a file changed on disk since it was read is refused (no merge) | crates/purlis-core/src/settings.rs:973 (`unchanged`) | `consents` |
| 12 | Shared-file / workspace-manifest writes are refused on a project opened read-only (newer schema, missing `requires`, unreadable manifest), except a whole-file save that mends an unreadable manifest | crates/purlis-core/src/settings.rs:924 (`save_as`), :950 (`writable`); crates/purlis-core/src/compat.rs:203 | `guards.plane` |
| 13 | A settings file (and `charter.toml` a chat's start reads) is capped at 1 MiB | crates/purlis-core/src/sandbox.rs:563 (`PLANE_FILE_MAX`); crates/purlis-core/src/settings.rs:350 | `limits` |
| 14 | Which list keys merge as sets across machines (sorted, deduped) vs key-by-key conflict | crates/purlis-core/src/tomlmerge.rs:25 (`SETS`) | `guards.plane` |
| 15 | A workspace manifest a hand edited (digest mismatch or unparseable) is never rewritten by purlis's automatic writers | crates/purlis-core/src/manifest.rs:56 (`ownership`) | `guards.plane` |
| 16 | What the generated `.claude/settings.json` in a guest checkout / workspace carries from the plane: only `enabledPlugins`, `env`, and `permissions.ask`/`deny` — never `allow` | crates/purlis-core/src/layer.rs:147 (`WORKSPACE_KEYS`), :155 (`RESTRICTIVE`), :443 (`restrictive`) | `guards.plane` |
| 17 | A generated file is purlis's to overwrite only when the sidecar digest still matches; anything else is the operator's and never rewritten or removed | crates/purlis-core/src/layer.rs:56 (`MARKER`), :175 (`key_ok`) | `guards.plane` |
| 18 | Which project `.claude/settings*.json` keys a sandboxed Claude chat loads; any other key → project files not loaded at all (deny/ask still carried) | crates/purlis-core/src/harness/claude/project.rs:31 (`KEPT`), :41 (`KEPT_RULES`), :44 (1 MiB) | `mode` |
| 19 | A project's Claude settings that change mid-chat and fail the screen restart the chat on the person's + purlis's settings; unwatched → not loaded | app/src-tauri/src/projectsettings.rs:1-17; crates/purlis-core/src/harness/claude/project.rs:125, :132 | `mode` |
| 20 | A Claude chat's purlis plugin is pinned on and the Python charter's (and former ids) pinned off, written last in `--settings` | crates/purlis-core/src/harness_plugin.rs:242 (`CLAUDE_PINS`); crates/purlis-core/src/harness/claude.rs:748 | `tools` |
| 21 | Files a chat may *propose* a change to (shown to the person to apply): plane manifest, a workspace's manifest, `personas/<p>/mcp.json` and `persona.md`, `.claude/skills/**` (no dot-parts, no `package.json`/`hooks`), no links, no new persona/workspace folder | crates/purlis-core/src/proposed.rs:72 (`proposable`), :218, :228 (`SKILL_NEVER`) | `guards.plane` |
| 22 | Proposal size: change ≤ 12 KiB, file ≤ 1 MiB, path ≤ 256 bytes | crates/purlis-core/src/proposed.rs:54, :58, :61 | `limits` |
| 23 | A Claude chat's Write/Edit/MultiEdit of a proposable file is turned into a proposal and the tool refused — only for a sandboxed Claude Code chat in the plane, app-numbered, policy not off | crates/purlis-core/src/toolhooks.rs:635 (`handed_over_from`), :662 (`proposing`) | `guards.plane` |
| 24 | `purlis propose <file> --from <draft>`: only in an app-started chat; draft must be in the chat's own folder; ≤ 12 KiB; UTF-8 | crates/purlis-cli/src/brokered.rs:264 (`propose`) | `guards.plane` |
| 25 | Proposed changes can be turned off on the machine (chat gets the plain refusal instead) | crates/purlis-core/src/sandbox/policy.rs:749 (`proposed_changes_refused`) | `guards.plane` |
| 26 | At most 8 proposals waiting per chat; a newer one for the same file replaces the older; no "always apply"; Apply only while file unchanged | app/src-tauri/src/proposals.rs:45 (`AT_MOST_PER_CHAT`), :397 (`asked_in`) | `limits` |
| 27 | A chat's direct Write/Edit into purlis's state directory (`.purlis/`) is denied | crates/purlis-core/src/toolhooks.rs:547 (`pretooluse_edit`), :540 | `fixed` |
| 28 | Brokered writes (memory, todo, vision, section, forget/edit/archive/move memory, optimize, todo close, workspace create, snapshot, check report) are a fixed typed set; none names a place except a *new* workspace | crates/purlis-core/src/brokered.rs:29 (`Write`) | `guards.plane` |
| 29 | A brokered write never touches anything under a sandbox-PLANTED name, any `.gitignore`, or a manifest/local settings change to `[sandbox]`, `[chat_env]`, `[dispatch]` | crates/purlis-core/src/brokered.rs:1306 (`guard`), :1288 | `fixed` |
| 30 | Brokered write text (text+title+names) ≤ 12 KiB; title one line | crates/purlis-core/src/brokered.rs:220 (`MOST_TEXT_BYTES`), :225 (`check`) | `limits` |
| 31 | Brokered write rate per chat: 60 a minute | crates/purlis-core/src/brokered.rs:1426 (`WRITES_A_MINUTE`); app/src-tauri/src/brokered.rs:30 | `limits` |
| 32 | A brokered write is answered only for a chat the app has open, at the place/persona the app recorded at start (never the request's) | app/src-tauri/src/brokered.rs:19 (`write`) | `guards.plane` |
| 33 | A brokered write from a chat on a pool's runner is held as a proposal for the person to Apply | app/src-tauri/src/brokered.rs:44-50 | `runners` |
| 34 | A brokered write waits at most 10 s for the app; a proposal hand-off 4 s | crates/purlis-cli/src/brokered.rs:78, :248 | `limits` |
| 35 | `workspace.md` grows to at most 64 KiB through a chat's vision/section writes (a hand-grown file is not refused) | crates/purlis-core/src/brokered.rs:1074, :1080 | `limits` |
| 36 | A section/vision entry that would hide what follows (open fence/raw block) is refused | crates/purlis-core/src/brokered.rs:1106 (`hides_nothing_after_it`) | `guards.plane` |
| 37 | A chat with no persona has no persona memory; persona memory goes only to its own persona or `_shared` | crates/purlis-core/src/brokered.rs:537 (`own_scope`), :558 (`own_persona`), :910 | `guards.plane` |
| 38 | `-w <other workspace>` on a writing workspace verb is refused in a sandboxed chat (own workspace only) | crates/purlis-cli/src/brokered.rs:622 (`named`) | `sandbox` |
| 39 | `purlis workspace create --live` / `--use` refused in a sandboxed chat | crates/purlis-cli/src/brokered.rs:546 (`create_refused`) | `sandbox` |
| 40 | Commands that rewrite a workspace's `workspace.json` are refused in a sandboxed chat | crates/purlis-cli/src/brokered.rs:593 (`manifest_refused`) | `sandbox` |
| 41 | MCP `todo_add` refuses an empty text and a near-duplicate (≥ 50% shared words) | crates/purlis-core/src/chattools.rs:1476-1488; crates/purlis-core/src/memstore.rs:569 (`DUPLICATE_THRESHOLD`) | `guards.plane` |
| 42 | An empty memory body is refused (a failed substitution must not land) | crates/purlis-core/src/memstore.rs:196; crates/purlis-core/src/chattools.rs:2051 | `guards.plane` |
| 43 | Memory title cut to 72 chars | crates/purlis-core/src/memstore.rs:23 (`TITLE_MAX`) | `limits` |
| 44 | A memory/plane file read whole is capped at 1 MiB (larger is not a memory) | crates/purlis-core/src/memstore.rs:1066 (`MAX_BYTES`) | `limits` |
| 45 | Session record shape: title ≤ 120 chars one line; body ≤ 32 KiB, exactly the five sections, no control chars, no secret-shaped text | crates/purlis-core/src/sessionrecord.rs:46, :49, :53, :249 (`check`) | `limits` |
| 46 | Session records per chat: 30 in all, 5 per 10 min | crates/purlis-core/src/brokered.rs:1522, :1526, :1529; app/src-tauri/src/smartclose.rs:307 | `limits` |
| 47 | Smart-close Notices per chat: 3 per 10 min, then log only | app/src-tauri/src/smartclose.rs:280, :283 | `limits` |
| 48 | The MCP tool set every chat is handed (33 tools), acting only on the chat's own place; no tool takes a workspace/path | crates/purlis-core/src/chattools.rs:638 (`TOOLS`), :1378 (`call`) | `tools` |
| 49 | Which harnesses are handed purlis's MCP server, and how | crates/purlis-core/src/harness/claude.rs:990 (`--mcp-config`); crates/purlis-core/src/harness/codex.rs:230 (`-c mcp_servers`); crates/purlis-core/src/opencode.rs:366 (config `mcp`); crates/purlis-core/src/acp.rs:1212 (`session/new`) | `tools` |
| 50 | Skill tools listed only where the project's skills reach this chat | crates/purlis-cli/src/mcp.rs:88, :92 | `skills` |
| 51 | A skill grade's chat is served only the read-only tools (`PRE_ALLOWED`); any other call refused | crates/purlis-cli/src/mcp.rs:93, :109; crates/purlis-core/src/chattools.rs:334 (`GRADED_ARG`) | `fixed` |
| 52 | Read-only purlis tools Claude runs without a prompt (18 named) | crates/purlis-core/src/chattools.rs:229 (`PRE_ALLOWED`); crates/purlis-core/src/harness/claude.rs:849 | `tools` |
| 53 | Own-scope writing tools Claude runs without a prompt (todo/memory/persona remember/workspace section+vision/ask_operator/worktree update+remove/secret_ask); not in a grade's chat | crates/purlis-core/src/chattools.rs:264 (`OWN_SCOPE_TOOLS`); crates/purlis-core/src/harness/claude.rs:839, :857 | `tools` |
| 54 | Dispatch tools run without the harness's prompt (consent is the dispatch grant) | crates/purlis-core/src/chattools.rs:595, :614; crates/purlis-core/src/harness/claude.rs:846; crates/purlis-core/src/chattools.rs:499 (Codex `approve`); crates/purlis-core/src/opencode.rs:383 (opencode `allow`) | `tools` |
| 55 | Codex waits on a purlis tool up to 3600 s | crates/purlis-core/src/chattools.rs:322 (`CODEX_TOOL_TIMEOUT_SECS`) | `limits` |
| 56 | A persona's `mcp.json` servers are started only on Claude Code; a credentialed one only after this machine approved its line; purlis's own server name cannot be taken | crates/purlis-core/src/personaverbs/chatstart.rs:68 (`servers`), :185 (`carries`); crates/purlis-core/src/harness/claude.rs:990 (written last) | `trust` |
| 57 | Approving a persona's MCP servers is refused from inside a chat | crates/purlis-core/src/personaverbs/approve.rs:44, :122 | `trust` |
| 58 | A persona declaring `disallowed-tools:` is not started on a harness that cannot enforce it (fail closed) | crates/purlis-core/src/personaverbs/chatstart.rs:194 (`unenforced`) | `tools` |
| 59 | Persona tool gate: a Bash command the active persona's `tools:` declares runs without prompt (never interpreters, never destructive subcommands; never denies) | crates/purlis-core/src/personagate.rs:1-30, :267, :367 | `tools` |
| 60 | Profile approval: a Local profile's command (kind+command+env) must be approved before it runs, and again when it changes; a built-in never asks | crates/purlis-core/src/profiletrust.rs:283 (`approval_needed`), :289 | `trust` |
| 61 | Harness declaration approval: a project's `harnesses/<name>.toml` (or Local `[harnesses.*]`) must be approved by digest, again on change | crates/purlis-core/src/profiletrust.rs:312; crates/purlis-core/src/harness_declaration.rs:54 (`harness-declarations-approved.json`) | `trust` |
| 62 | Any unreadable/missing/oversized (>1 MiB) approval record reads as not approved | crates/purlis-core/src/profiletrust.rs:105, :127 | `trust` |
| 63 | Approve records only what was shown (refused if the profile changed while read) | crates/purlis-core/src/profiletrust.rs:351 (`approve`) | `trust` |
| 64 | A dispatched chat never starts on a profile that still needs approval (nobody stands in front of it) | crates/purlis-core/src/personaprofile.rs:194 (`Refused`), :452 (`for_dispatch`) | `trust` |
| 65 | A persona's `profile:` (or legacy `model:`) names only a profile the project offers on this machine, never a program | crates/purlis-core/src/personaprofile.rs:46, :49 | `trust` |
| 66 | A harness declaration may not name a shell/interpreter/launcher as its program (incl. versioned names) | crates/purlis-core/src/harness_declaration.rs:1858 (`LAUNCHERS`), :2050, :2060 | `trust` |
| 67 | A declaration may not set loader/injector variables or tool-steering variables (TLS trust, proxies, SSH agent, pager/editor/compiler) | crates/purlis-core/src/harness_declaration.rs:2092 (`INJECTORS`), :2126 (`STEERS`), :2221 (`env_ok`) | `trust` |
| 68 | Declaration file ≤ 64 KiB; drawn words ≤ 60/80/200 chars | crates/purlis-core/src/harness_declaration.rs:58, :62, :65, :68 | `limits` |
| 69 | Which credential variables a profile's sign-in may hand a harness, per kind; only by vault reference | crates/purlis-core/src/profilesignin.rs:142 (`handed`), :99 | `vaults` |
| 70 | Cloud sign-in refused for Codex/Azure and Vertex; cloud not handed in this version; an api-key profile waits for the key proxy (does not start) | crates/purlis-core/src/profilesignin.rs:656 (`handing`) | `trust` |
| 71 | A chat's model: the profile's command (`--model`) or harness default; a sandboxed Claude chat may take `model` from the project's `.claude/settings.json` | crates/purlis-core/src/harness/claude.rs:1245; crates/purlis-core/src/harness/claude/project.rs:31; docs/adr/0087 (D-2036-6) | `trust` |
| 72 | A chat whose profile hands vault credentials is not started in the session host; a person's sandbox opt-out is not sent to a runner | app/src-tauri/src/hostedsessions.rs:130, :4904, :142, :4952 | `vaults` |
| 73 | Chats run in the session host unless `PURLIS_CHATS_IN_THE_HOST=0` | app/src-tauri/src/hostedsessions.rs:82 (`SWITCH`) | `runners` |
| 74 | Extension approval: an installed extension contributes nothing until the person approves its fingerprint (every file in its dir); any change asks again | crates/purlis-core/src/extension.rs:1017 (`standing`), :2715 (`approve`) | `trust` |
| 75 | Built-in extensions are approved without a prompt only at the app's bundle path; an install with a built-in's id is refused | crates/purlis-core/src/extension.rs:1027-1029, :2672 (`install`) | `trust` |
| 76 | A themes-only extension needs no approval (runs nothing) | crates/purlis-core/src/extension.rs:825, :830 | `trust` |
| 77 | A project may only turn on/configure an extension this machine approved, never install/approve one; approved with no layer naming it → on | crates/purlis-core/src/extension/project.rs:11-26, :419 | `trust` |
| 78 | Capability vocabulary is closed: an unknown capability word refuses the whole manifest; studio words known only with the `skill-studio` flag | crates/purlis-core/src/extension/capability.rs:260, :265, :290 | `trust` |
| 79 | A studio-capability extension is confined: reads its own folder only, writes only its state + temp, network only to declared hosts via its own proxy, never a model host; refused where no sandbox exists | crates/purlis-core/src/extension/studio.rs:15-30, :138 (`refused_here`), :471 (`model_host`) | `trust` |
| 80 | A non-studio extension is not confined ("runs as you") | crates/purlis-core/src/extension.rs:321 (`RUNS_AS_YOU`) | `trust` |
| 81 | Extension size bounds: manifest 64 KiB, kept file 8 MiB, 64 declared files, 4096 tree entries, 64 MiB hashed, record 1 MiB, 16 settings of ≤ 200 B, 8 views, 16 actions/commands | crates/purlis-core/src/extension.rs:262, :271, :276, :288, :295, :301, :588, :591, :765; crates/purlis-core/src/extension/acting.rs:26, :30 | `limits` |
| 82 | A router extension reads prompts only with the person's per-machine switch | crates/purlis-core/src/extension.rs:2803 (`set_reads_prompts`) | `trust` |
| 83 | First-open consent: a project's plugins, env and own-program chats are shown and approved per machine before opening | crates/purlis-core/src/machine.rs:572 (`Contribution`); app/src-tauri/src/opener.rs:590 (`approve_plane`) | `trust` |
| 84 | `theme_preview`: at most 2 specimens a call, 5 s each, 16 option values, 16 MiB font bytes, one chat turn per 5 s gap; never the person's window | crates/purlis-core/src/themepreview.rs:56, :69, :72, :90, :94; crates/purlis-core/src/themepreview/spacing.rs:22 | `limits` |
| 85 | `theme_preview` resolves theme ids only where the chat works and only its own drafts (`draft:` never another chat's) | crates/purlis-core/src/themepreview.rs:23-34 | `guards.plane` |
| 86 | Theme drafts: written by the app for the chat (never by the chat), ≤ 8 files and 64 KiB JSON per save, ≤ 32 chats per place; Keep/Revert are the person's | crates/purlis-core/src/theme/drafts.rs:79, :865, :870; app/src-tauri/src/themedrafts.rs:13 | `limits` |
| 87 | Curation actions: prompt typed as one bracketed paste and never sent (no control chars that could submit) | crates/purlis-core/src/curation.rs:308 (`pasted`), :319 | `guards.shape` |
| 88 | Built-in curation actions fixed (safe-remove, compact, add-curation-action, create-theme); a persona's file cannot override or impersonate one; persona `charter`/`purlis` reserved | crates/purlis-core/src/curation.rs:252 (`BUILTINS`), :345 (`parse`); crates/purlis-core/src/personas.rs:268 (`RESERVED`) | `guards.plane` |
| 89 | Curation template variables limited to four; any other `{word}` refused | crates/purlis-core/src/curation.rs:339 (`VARIABLES`) | `guards.shape` |
| 90 | Removing a workspace from a sandboxed chat is never done; the name is handed to the person as a question (fallback refusal); persona removal from a sandboxed chat refused outright | crates/purlis-cli/src/brokered.rs:480, :463, :500; crates/purlis-core/src/removalask.rs:26 | `guards.plane` |
| 91 | A delete is held in a chat whose sandbox holds the plane's manifests or a file below the target | crates/purlis-cli/src/brokered.rs:570 (`removal_held`) | `guards.plane` |
| 92 | Workspace remove is refused while any chat is open in it (even with `--force`); refused over work at risk unless `--force` | crates/purlis-core/src/wscmd/remove.rs:147, :153; crates/purlis-core/src/wscmd/mod.rs:614 (`work_at_risk`) | `guards.plane` |
| 93 | `worktree_remove` for a chat: only a branch folder of its own workspace, from above it (never the one it stands in), never forced, refused over uncommitted/unique commits or another live chat's folder | crates/purlis-core/src/gitbroker.rs:183-188, :326; crates/purlis-core/src/hookwire.rs:2183 | `guards.plane` |
| 94 | `purlis worktree remove --force` refused in any app-started chat; plain removal refused in a sandboxed chat when no app took it | crates/purlis-cli/src/brokered.rs:528, :512 | `guards.plane` |
| 95 | Explorer's workspace-file writes (new/rename/trash/import) are window-only; no chat or link runs them | app/src-tauri/src/workspacewrites.rs:18; crates/session-protocol/src/ui.rs:203 (`WINDOW_ONLY`) | `consents` |
| 96 | Window-only command list (Apply proposal, approve extension/profile/plane, remove workspace/persona, theme Keep/Revert, …): never served on a link | crates/session-protocol/src/ui.rs:203 (`WINDOW_ONLY`), :443, :567, :570, :584, :586, :589 | `consents` |

### Other (B)

| # | Decision | Where | Family |
|---|---|---|---|
| 1 | Where a chat runs: start choice > workspace > project > this machine (order) | crates/purlis-core/src/machine.rs:1912 (`Store::chat_place`) | `runners` |
| 2 | A chat at the project root can never run on a pool | crates/purlis-core/src/pools.rs:386; app/src-tauri/src/handoff.rs:323 (dispatch) | `runners` |
| 3 | A start naming a pool this machine doesn't keep is refused, never run here instead | crates/purlis-core/src/pools.rs:415 (`runner_of` check in `chat_place`); app/src-tauri/src/handoff.rs:3719 (`names_a_pool_kept`) | `runners` |
| 4 | An unreadable machine store refuses a workspace start (fail closed) | crates/purlis-core/src/pools.rs:403 | `runners` |
| 5 | At most 64 workspaces of one project may name a pool | crates/purlis-core/src/machine.rs:215 (`MOST_CHATS_RUN_ON`), enforced :1710 and :3165 | `limits` |
| 6 | A chat's dispatch that names a run place other than its target's own levels needs the person's Approve, whatever grant/persona covers it | app/src-tauri/src/handoff.rs:3745 (`moves_work`), comment :312 | `dispatch` |
| 7 | An open/dispatch heard from a chat on a pool's runner is held for the person; dispatch grants don't carry it; no "always approve" | app/src-tauri/src/runnerdispatches.rs:291 (`heard_from_a_pool`), :297 (`hold`); handoff.rs:304 | `runners` |
| 8 | Held runner dispatches: at most 8 per chat, 8 per pool | app/src-tauri/src/runnerdispatches.rs:29, :33 | `limits` |
| 9 | A runner chat's write to the project (persona, workspace, memory) is held as a proposal; only the person applies; no "always apply" | app/src-tauri/src/runnerwrites.rs:1 (module), `HELD_SAID` :35 | `runners` |
| 10 | Held runner writes: at most 16 per chat | app/src-tauri/src/runnerwrites.rs:32 | `limits` |
| 11 | A held runner write carrying a credential or binary is refused, never held | crates/purlis-core/src/runnerwrite.rs:176 (`refusal`) | `runners` |
| 12 | Every run of a runner chat's declared tool that needs vault values asks the person, per run; no standing Allow | app/src-tauri/src/pooltools.rs:289 (`checked`), :468 (`decided`) | `vaults` |
| 13 | A pool tool's run waits for the person at most 15 min | crates/purlis-core/src/hookwire/relay.rs:288 (`A_HANDED_LINE_WAITS_AT_MOST`) | `limits` |
| 14 | No vault value, token, browser login or profile sign-in goes to a runner not shown to be the person's own (local Docker engine socket; remote/cluster pool only with the person's "This is my own" mark for the current pin; cluster vouched now) | crates/purlis-core/src/poollogins.rs:432 (`owned`), :446 (`marked_as_pinned`), :293 (`over_the_link_with_kinded`), :209/:236 (cluster); crates/purlis-core/src/pools.rs:2957 (`cluster_runner_yours`), :3003 (`cluster_runner_still_yours`); app/src-tauri/src/pooltools.rs:289 | `vaults` |
| 15 | A pin change since the mark takes the mark away (values stop until re-marked) | crates/purlis-core/src/poollogins.rs:457; crates/purlis-core/src/runners.rs:321 (`Pool::marked`) | `vaults` |
| 16 | A persona's browser login on a pool needs the person's opt-in, per persona+login+pool, re-asked if the pool/runner changed; in memory only | crates/purlis-core/src/poollogins.rs:533 (`allow`), :539 (`keep_off`), :550 (`standing`) | `browser` |
| 17 | Runner taint: once any non-shell/non-browser program ran on a runner outside landlock/bubblewrap/seatbelt, that runner gets no browser, no login, no vault value/token, no sign-in until remade; fail closed on unreadable file | crates/purlis-core/src/runnertaint.rs:61 (`taints`), :107, :200; uses at poollogins.rs:213/:313, pools.rs:3012, poolsetup.rs:1733, windowline.rs:247 | `runners` |
| 18 | Sign-ins are not carried to a remade runner, and conversations not brought back, where either runner was tainted or its home isn't its own | crates/purlis-core/src/pools.rs:1301, :1718, :1919 | `vaults` |
| 19 | A profile being signed out of a pool is held from every drain's carry | crates/purlis-core/src/runners.rs:1766, :1816 | `runners` |
| 20 | Pool limit: most a profile may send, size | crates/purlis-core/src/poollimits.rs:216 (ALL, `profileSize`) | `limits` |
| 21 | Pool limit: profile files | crates/purlis-core/src/poollimits.rs:216 (`profileFiles`) | `limits` |
| 22 | Pool limit: largest code sent to a runner | crates/purlis-core/src/poollimits.rs:216 (`codeSize`) | `limits` |
| 23 | Pool limit: longest code send | crates/purlis-core/src/poollimits.rs:216 (`codeSendTime`) | `limits` |
| 24 | Pool limit: longest wait for a sign-in | crates/purlis-core/src/poollimits.rs:216 (`signInWait`); runners.rs:1139 | `limits` |
| 25 | Pool limit: wait for cluster runner to be vouched for (clusters only; never team-set) | crates/purlis-core/src/poollimits.rs:216 (`vouchWait`); pools.rs:2287 (`VOUCHED_WITHIN` 300 s) | `limits` |
| 26 | Pool limit: wait for a runner's answer | crates/purlis-core/src/poollimits.rs:216 (`answerWait`) | `limits` |
| 27 | Docker runner CPUs | crates/purlis-core/src/poollimits.rs:216 (`cpus`); crates/purlis-core/src/dockerpool.rs:467 (`Limits::default`), :494 (`Limits::new` bounds) | `limits` |
| 28 | Docker runner memory | poollimits.rs:216 (`memory`); dockerpool.rs:467, :484 (`LEAST_MEMORY_MIB`) | `limits` |
| 29 | Docker runner processes | poollimits.rs:216 (`processes`); dockerpool.rs:467, :480 (`FEWEST_PIDS`) | `limits` |
| 30 | Docker runner /tmp size (≤ memory) | poollimits.rs:216 (`tmp`); dockerpool.rs:467 | `limits` |
| 31 | Runner suspends itself after this long with no desktop link | poollimits.rs:216 (`suspendAfter`); dockerpool.rs:575 (`SUSPEND_AFTER`), :579/:582 (1 min..30 days); crates/host/src/unattended.rs:32 (`UNLINKED_FOR`) | `limits` |
| 32 | A cluster team's default only lowers a limit, never raises it unasked; team highest caps what the person may set; runner-shaping and vouchWait not team-settable | crates/purlis-core/src/poollimits.rs:800 (`default_of`), :809 (`team_suggests`), :964 (`under_team`) | `limits` |
| 33 | A chat may propose a pool limit change; person applies; refused where proposals are locked off | app/src-tauri/src/proposals.rs:470 (`limit_asked_in`); crates/purlis-core/src/runners.rs:1127 (`limit_may_be_set`) | `runners` |
| 34 | Fixed runner limits: stop/re-arm wait per runner; most of a runner's output read | crates/session-protocol/src/fixedlimits.rs:34 (`STOP_WAIT`), :42 (`RUNNER_OUTPUT`), :50 | `limits` |
| 35 | Cluster Pool: runner idle before suspend | operator/src/resources.rs:53 (`idle_after`), :133; crates/purlis-core/src/clusterpool/idle.rs:16, :30 (`PARKED_FOR` 5 min) | `limits` |
| 36 | Cluster Pool: kept Claim deleted after suspended / notice before / max age | operator/src/resources.rs:59, :65, :72 | `limits` |
| 37 | Cluster Pool stopped: every runner suspended, new Claims get none | operator/src/resources.rs:46 (`stopped`) | `limits` |
| 38 | Cluster Pool warm runners | operator/src/resources.rs:86 (`warm_replicas`) | `limits` |
| 39 | Per-person cluster quota (claims, CPU, memory, storage) | operator/src/person.rs:320 (`Quota::default`), :63 (`PURLIS_PER_PERSON_QUOTA`) | `limits` |
| 40 | Cluster forge tokens: repos a token may cover; opt-in to mint in cluster; permissions mintable | operator/src/resources.rs:104 (`repos`), :108 (`mint_in_cluster`); operator/src/github.rs:41 (`MINTABLE`); crates/purlis-core/src/clusterpool.rs:2133 (`MOST_CLAIM_REPOS` 256); operator/src/forge.rs:70 (`MOST_REPOS` 500) | `guards.credential` |
| 41 | Kill switch: stopped refuses every start on this host and its runners; fails closed; only the window re-arms; `purlis stop --all` stops, never re-arms | crates/host/src/killswitch.rs:118 (`is_stopped`), :133 (`refusal`); crates/purlis-core/src/halt.rs:150 (`stopped_on_disk`), :208 (`stop`), :230 (`rearm`); app/src-tauri/src/killswitch.rs:585, :602 | `fixed` |
| 42 | Re-arm reaches the runners the stop stopped | app/src-tauri/src/killswitch.rs:252 (`runners_a_rearm_reaches`) | `fixed` |
| 43 | Relay: a command is never re-sent after a link drop; while no link is up every command is refused; a client's own `take_back` is refused | crates/host/src/relay.rs:28, :44 (doc), :1231 | `runners` |
| 44 | A runner chat's `secret exec` is held to the runner host's record of its sandbox, live reach included | crates/purlis-core/src/hostedchats.rs:74 (`record`), :130 (`asker`) | `vaults` |
| 45 | Per-session plane state kept 30 days after last write; live/returning chats' files kept however old | crates/purlis-core/src/retention.rs:62 (`KEEP_FOR`), :92 (`sweep`), :314 (`on_open`) | `limits` |
| 46 | A trace recording a secret hand-out is kept; trace >64 MiB kept unread | crates/purlis-core/src/retention.rs:65 (`MOST_TRACE_BYTES`) | `limits` |
| 47 | Dispatch record's talk kept 30 days after end | crates/purlis-core/src/dispatchrecord.rs:980 (`TALK_KEPT_FOR`) | `limits` |
| 48 | Chat records kept: at most 2000 | crates/purlis-core/src/chatrecords.rs:54 | `limits` |
| 49 | Chat screens kept: 200 / 256 MiB | crates/purlis-core/src/chatscreens.rs:54, :57 | `limits` |
| 50 | Sandbox block record: 30 days / 5000 lines | crates/purlis-core/src/sandboxblock_record.rs:54, :65 | `limits` |
| 51 | Runner hours kept 14 days | crates/purlis-core/src/runnerhours.rs:133 | `limits` |
| 52 | Usage ring: last 16 turns per session | crates/purlis-core/src/usage.rs:52 (`KEEP`) | `limits` |
| 53 | purlis sets no token/cost/turn limit of its own; a vendor usage limit ends the turn and its reset time is recorded for Resume | crates/purlis-core/src/usage.rs:670 (`limit_resets`), :689 (`record_limit`); crates/purlis-core/src/state.rs:220 (`rate_limit` → `UsageLimit`); app/src-tauri/src/skillgrade.rs:2627 | `limits` |
| 54 | Every clone's local config: forge CLI credential helper, `commit.gpgsign=false`, `tag.gpgsign=false`, SSH→HTTPS insteadOf | crates/purlis-core/src/gitpolicy.rs:62 (`apply`) | `guards.git` |
| 55 | Every chat's git is armed with purlis hooks via `core.hooksPath` env pair (pre-commit, pre-merge-commit scan; pre-push scan; commit-msg trailers); repo hooks run after | crates/purlis-core/src/githooks.rs:59 (`CHECKED`), :68, :75, :121 (`GitHooks`) | `guards.git` |
| 56 | Bash guard refuses skipping hooks: `--no-verify`, `-n` on commit, `git send-pack`, `-c core.hooksPath`, `GIT_CONFIG_*`, revert/ff to a changed scan allowlist, `doctor --fix rename-plane` | crates/purlis-core/src/commitguard.rs:168 (`hook_skip_hit`), :127, :292 | `guards.save` |
| 57 | A sandboxed chat's `git add`/`commit` in a purlis branch folder is refused with the retry shape (`purlis worktree commit`) | crates/purlis-core/src/branchgit.rs:183 (`branch_write`) | `sandbox` |
| 58 | A sandboxed chat's git that moves a clone/branch folder to another commit is refused where it would write a sandbox-denied path | crates/purlis-core/src/cloneupdate.rs:1 (arm), :1874 (`MAX_FOLDERS` 32); crates/purlis-core/src/pieceupdate.rs:22 | `sandbox` |
| 59 | Brokered git asks are bound to where the chat stands: its own workspace; update/push at or above the folder; remove only from above, never the folder it stands in; root update only for the root chat; root save only where the chat can't write root git | crates/purlis-core/src/gitbroker.rs:176-206, :679 (`bound`), :707, :727 (`above`), :751 (`at_the_root`), :765 | `guards.git` |
| 60 | Brokered fetch/push/clone reach only hosts the chat's sandbox allows | crates/purlis-core/src/gitbroker.rs:928 (`hosts`); pieceupdate.rs:76 (`Reach`) | `network` |
| 61 | A repo whose config names a program, a signer, alternates or `config.worktree` is refused before git runs | crates/purlis-core/src/gitbroker.rs:1138 (`runs_a_program`), :1180, :1246, :1274 | `guards.git` |
| 62 | Brokered commit: only commit (no amend/reset/rebase/merge/push), no editor, no signing, repo's pre-commit not run, purlis scan runs; refuses gitlinks, `.gitmodules`, case twins, filtered files, hard links, purlis provenance trailers | crates/purlis-core/src/gitbroker/commit.rs:88 (`COMMITS_ONLY`), :455 (`trailers_ok`), :1229, :1272, :1324 | `guards.git` |
| 63 | Brokered commit caps: message 32 KiB, 1000 paths, 4096-byte path; stage 5 min, commit 2 min | crates/purlis-core/src/gitbroker/commit.rs:79, :82, :85, :110, :113 | `limits` |
| 64 | Brokered commit rate: 30 per chat per 10 min | crates/purlis-core/src/gitbroker/commit.rs:416, :419 | `limits` |
| 65 | An unsandboxed chat may be told to commit with plain git instead | crates/purlis-core/src/gitbroker/commit.rs:101 (`refuses_unsandboxed`), :201; crates/purlis-core/src/planesave.rs:164 (default false); crates/purlis-cli/src/gitask.rs:307 | `guards.git` |
| 66 | One live writer per branch folder: a second chat's commit/update/push/remove is refused naming the holder; brokered fails closed, plain git (guard) fails open when no app answers | crates/purlis-core/src/piececlaim.rs:168 (`writer`), :320 (`take`), :415 (`removing`); gitbroker.rs:280, :339, :1100; crates/purlis-cli/src/guard.rs:304; crates/purlis-cli/src/gitask.rs:355 | `guards.git` |
| 67 | Claim lock waits at most 5 s | crates/purlis-core/src/piececlaim.rs:281 | `limits` |
| 68 | Worktree update: refused with uncommitted changes; only the piece's own commits move; never the project root folder (for a branch-folder update); `--merge` never rewrites | crates/purlis-core/src/pieceupdate.rs:110 (`update`), :153 (`update_clone`); crates/purlis-cli/src/piece.rs:555 (`AT_THE_ROOT`), :578 | `guards.git` |
| 69 | Worktree push: own branch, own name, after purlis scan; refused if it changes the scan allowlist; overwrite only leased to a commit purlis's own update noted; no tag/push-option/submodule; no force push needed or done | crates/purlis-core/src/pieceupdate.rs:1831 (`pushed`), :1884-:1890; crates/purlis-core/src/change/push.rs:82, :87 (cross-repo) | `guards.git` |
| 70 | Worktree remove from a chat: never forced; refused for uncommitted changes, unique commits, own git state, unknown state, a task's folder, another live chat's folder | crates/purlis-core/src/worktree/mod.rs:227 (`Uncommitted`), :269 (`WouldLoseWork`), :282 (`OwnState`), :1302 (`would_refuse`); gitbroker.rs:321-:345; crates/purlis-cli/src/piece.rs:640 | `guards.git` |
| 71 | Worktree prune: only finished folders (task ended, no open undeclared holder, nothing lost, no links in) | crates/purlis-core/src/prune.rs:68 | `guards.git` |
| 72 | A chat never merges a pull/merge request: the hold refuses it in any mode and hands it to the app; only the person's two presses merge, at the head shown, only repos in the inventory; no grants for merges | crates/purlis-core/src/mergehold.rs:213 (`held`), :595 (`listed_in`); crates/purlis-core/src/mergehold/looked.rs:24 (`PRESSES`=2); crates/purlis-core/src/askrules.rs:337 (`make` refuses merge rules); crates/purlis-core/src/floorguard.rs:170 (`merge_reason`) | `guards.merge` |
| 73 | Merge asks held per chat: at most 3; forge read within 1.2 s | crates/purlis-core/src/mergehold.rs:80, :77 | `limits` |
| 74 | Unattended runs may not publish: `gh/glab release create`, `gh pr merge`, `glab mr merge`, `charter change land`, `charter skill publish` denied | crates/purlis-core/src/floorguard.rs:78 (`PUBLISH_FORGE`), :89 (`PUBLISH_SINCE`), :161 | `guards.release` |
| 75 | Cross-repo land: one member per run; needs landed first; checks PASSED at the head; merge pinned to that head; through queue/train where one exists; never forge auto-merge | crates/purlis-core/src/change/land.rs:119 (`land`), :1117 (`verdict`) | `guards.merge` |
| 76 | Cross-repo change push refuses a repo that rewrites push URLs; branch names starting with `-` refused; record text ≤1024 | crates/purlis-core/src/change/push.rs:82; crates/purlis-core/src/change/record.rs:101, :32 | `guards.git` |
| 77 | Protected branch push classed high-risk (repo default branch + `[risk] protected_branches`): never grouped, never a rule, Allow confirmed twice | crates/purlis-core/src/askrisk.rs:109 (`Facts::read`), :49 | `asks` |
| 78 | Repo save in a request mode never pushes to the default branch or PR base | crates/purlis-core/src/reposave.rs:26 (doc) | `guards.save` |
| 79 | A save the app makes for a chat brings nothing in (`--pull` refused) | crates/purlis-core/src/gitbroker.rs:816 (`SAVE_PULL_FOR_A_CHAT`) | `guards.save` |
| 80 | Commitment nudge on prompt submit: never unattended (attendance fact), never on lookup/slash/no-fork, first prompt or code-change ask, then quiet 3 prompts | crates/purlis-core/src/commitgate.rs:214 (`nudge`), :62 (`COOLDOWN`) | `guards.save` |
| 81 | Brokered git waits: worktree ask, piece update, commit, save | crates/purlis-cli/src/gitask.rs:44, :126, :142, :132 | `limits` |
