# What each harness can enforce itself, and what purlis enforces for it

Research, 2026-11-16, for #6972 (part of the map #6969, ruled by Q307). Facts only. This document
decides no policy. It is the per-harness list of known gaps that Q307's Q5 needs: where a policy
key can't be enforced the same way in every harness, and what purlis does there today.

Read from `main` at `eb9c63fe8`. No harness was run.

**Pins** (`tests/fixtures/harness-versions/*.txt`): Claude Code **2.1.300**, Codex CLI **0.147.0**,
OpenCode **1.18.33**.

**Marks.** Every claim carries one:

- **V**: read in purlis's code at the cited `file:line`.
- **S**: read in the harness's source at the pinned tag.
- **D**: a document: purlis's own (`docs/`, `crates/purlis-core/docs/`) or the harness's official
  docs.
- **I**: inferred from the verified facts, not read or seen directly.

**Path prefixes.**

- `core/` is `crates/purlis-core/src/`. `cdocs/` is `crates/purlis-core/docs/` (what `purlis docs
  show` prints).
- `codex:` is `openai/codex` at tag `rust-v0.147.0`, under `codex-rs/`.
- `oc:` is `sst/opencode` at tag `v1.18.33`, under `packages/`.
- `cc:<page>` is `https://code.claude.com/docs/en/<page>.md`, fetched 2026-11-16.

**Where the sources fall short of the pin.**

- **Claude Code** ships as a minified native binary, so every Claude Code claim is **D**. Its docs
  describe 2.1.325, 25 releases past the pin. Where a page dates a feature, the date is checked
  against 2.1.300. `blockWritesOutsideWorkingDirectories` (2.1.314) and
  `permissions.denyInsteadOfPrompting` (2.1.319) are **not** in the pin. A claim the docs do not
  date is assumed present (**I**).
- **Codex's** in-repo docs are stubs that point to developers.openai.com, which this chat's
  network sandbox refused. json.schemastore.org was refused too. The Codex facts are therefore
  from source.
- **OpenCode's** live docs at opencode.ai/docs already describe its V2 config (`permissions`,
  `shell`, `subagent`), not 1.18.33's V1 (`permission`, `bash`, `task`). The facts here come from
  source and from the `.mdx` docs in the repo at the tag.

## The answer in one screen

| | Claude Code | Codex | OpenCode |
|---|---|---|---|
| Who confines the chat's commands | **Claude Code's own sandbox**, configured by purlis through `--settings` (macOS). On Linux it is off, and purlis's wrap holds the whole process | **purlis's wrap** around the whole process. Codex's own sandbox is off (`--sandbox danger-full-access`) | **purlis's wrap** around the whole process. OpenCode has no sandbox |
| Who confines its built-in file tools | Claude Code's **in-process** permission check (`permissions.deny` `Read(...)`/`Edit(...)`). It is not OS-level | the wrap | the wrap |
| Where its hooks and MCP servers run | **outside** any sandbox | inside the wrap | inside the wrap (in OpenCode's JS realm) |
| Hooks purlis gets, and which can block | the full set (about 20). `PreToolUse` blocks on Bash, Read/Grep, Edit and dispatch | 7 words. Only Bash `PreToolUse` blocks. No Read, Edit or dispatch hook | a TS plugin shim. It blocks bash, read, grep, write, edit and task by throwing |
| A hook that times out | **the tool runs** | **the tool runs** | the shim **refuses** |

Sources: V `core/sandbox/claude.rs:4-24`; V `core/sandbox/claude_wrap.rs:4-29`; V
`core/sandbox/codex.rs:4-24`; V `core/sandbox/opencode.rs:4-5`; V `core/hookreg.rs:68-224`; V
`core/plugin.rs:130-170`; V `app/src-tauri/plugin/opencode/purlis.ts:21-66`; D
`cdocs/hooks.md:978-983`; D `docs/adr/0124-who-judges-a-chat-and-how-much.md:615-626`.

**The gaps, in short.** Each gap is detailed under its family below, and the table at the end lists
them all.

- **Codex:** no Read/Edit/dispatch hook, so the vault and state-folder guards are missing there.
  There is nowhere for a `guard ask` rule. A sandboxed chat rejects where it would ask. Its hooks
  are inert until the TUI trusts them, and `write_stdin` is not hooked.
- **OpenCode:** no sub-agent dispatch refusal, and `ask_operator` cannot ask. The shim shares one JS
  realm with any other plugin.
- **Claude Code:** its hooks and non-purlis MCP servers run outside the sandbox, with open network.
  Its file tools are held by an in-process check. The keychain cannot be denied on macOS. A
  Linux chat has no listen door.
- **All three:**
  - No OS limit on cross-workspace reads.
  - The harness's sign-in is not kept from the commands it runs.
  - A hook that times out lets the tool run on Claude Code and Codex.
  - A chat with its sandbox lifted can rewrite the policy that governs it. Q4 says it never may.

## 1. What each harness enforces natively

### 1.1 Side by side

| Capability | Claude Code 2.1.300 | Codex 0.147.0 | OpenCode 1.18.33 |
|---|---|---|---|
| Rule language | `permissions.allow`/`ask`/`deny` of `Tool(spec)`. Deny, then ask, then allow; the first match wins. Lists merge across scopes. D `cc:permissions` | `approval_policy` (`untrusted`, `on-request`, `granular`, `never`), plus Starlark `prefix_rule` (`allow`/`prompt`/`forbidden`, the strictest wins), plus a per-MCP-tool `approval_mode`. S `codex:protocol/src/protocol.rs:917-958`, `codex:execpolicy/src/decision.rs:9-16` | `permission` map: key → `allow`/`ask`/`deny` or `{glob: action}`. The **last** matching rule wins, and no match means `ask`. S `oc:opencode/src/permission/index.ts:28-38` |
| Shell matching | Glob on the command text, split on `&& \|\| ; \|` and so on. Says the docs: "not a security boundary" (`/bin/rm`, `sh -c` get past it). D `cc:permissions` | Token prefix of each command after splitting `bash -lc`. An **`allow` match also runs the command outside Codex's sandbox**. S `codex:core/src/exec_policy.rs:408-421, 837-885` | Raw text of each tree-sitter command node, wildcard-matched. `eval`, `bash -c`, `/bin/rm` and `$VAR` get past it (I). S `oc:opencode/src/tool/shell.ts:119-125, 392-410` |
| File path rules | `Read(path)`/`Edit(path)`, gitignore-style. They also cover the shell commands it recognises (`cat`, `sed`, `tee`, redirection), but not `grep -r` or scripts. D `cc:permissions` | None as rules. Only sandbox writable roots and `deny` entries in a permission profile. S `codex:config/src/permissions_toml.rs:224-245` | `read`/`edit` patterns, **relative to the worktree**, plus `external_directory` for paths outside. S `oc:opencode/src/tool/read.ts:255-260`, `tool/external-directory.ts:15-45` |
| OS sandbox | Seatbelt on macOS; bubblewrap and seccomp on Linux. Off by default. **Shell commands only**: the file tools, WebFetch, hooks, MCP servers and mods run outside it. D `cc:sandboxing` | Seatbelt on macOS; bubblewrap and seccomp on Linux. Covers **shell and `apply_patch`**, not MCP servers, hooks or `notify`. S `codex:core/src/tools/orchestrator.rs:208-235`, `codex:rmcp-client/src/stdio_server_launcher.rs:277,552`, `codex:hooks/src/engine/command_runner.rs:60-66` | **None.** The shell inherits the full user and environment. S `oc:opencode/src/tool/shell.ts:293-310, 416-426` |
| Restrict reads | Yes: `filesystem.denyRead`/`allowRead`, `blockReadsOutsideWorkingDirectories` (≥2.1.257), `sandbox.credentials`. D `cc:sandboxing`, `cc:permissions` | Yes: `deny` entries in a permission profile. A deny-read also stops `allow` and escalation from dropping the sandbox. S `codex:core/src/tools/sandboxing.rs:236-293` | Only the `read` permission on its own tool. A shell `cat` is not covered. S (absence) |
| Network for the shell | A hostname-allowlist proxy plus an OS block. `httpProxyPort`/`socksProxyPort` swap in another proxy. D `cc:sandboxing` | On/off (`network_access`), or per domain through its managed proxy. S `codex:core/.../seatbelt.rs:283-321`; D `codex:network-proxy/README.md:52-61` | **None.** S (absence) `oc:core/src/v1/config/config.ts` |
| Network for its own tools | `WebFetch(domain:…)` rules only. The sandbox proxy does not apply. D `cc:sandboxing` | `web_search` mode only (`disabled`, `cached`, `indexed`, `live`). S `codex:protocol/src/config_types.rs:357-363` | `webfetch` and `websearch` take **one** action each, with no URL rules. Redirects are not re-checked (I). S `oc:core/src/v1/config/permission.ts:29-30` |
| Unix sockets and local ports | `allowUnixSockets` (a path list, **macOS only**; on Linux it is all or none). `allowLocalBinding` is macOS only. D `cc:settings-reference` | Socket allow entries or all; `allow_local_binding`. S `codex:sandboxing/src/seatbelt.rs:240-298` | n/a |
| Blocking hooks | `PreToolUse` (exit 2 or `permissionDecision`), `UserPromptSubmit`, `Stop`, `ConfigChange` and more. They fire for subagents and MCP tools. **Fail-open** unless `"onFailure": "block"` (≥2.1.295). D `cc:hooks` | 11 events. `PreToolUse`, `PermissionRequest`, `UserPromptSubmit` and `Stop` block. **Fail-open** on timeout, crash or bad JSON. A hook runs only once its hash is trusted. **`write_stdin` emits no `PreToolUse`.** S `codex:hooks/src/config/hook_config.rs:35-59`, `codex:hooks/src/events/pre_tool_use.rs:270-295`, `codex:hooks/src/engine/discovery.rs:541-583`, `codex:core/src/unified_exec/write_stdin.rs:101-106` | Plugin `tool.execute.before` blocks by throwing. **No timeout**, so a hung plugin hangs the call. `permission.ask` is declared but never called. `event` is fire-and-forget. S `oc:opencode/src/plugin/index.ts:255-297`, `oc:plugin/src/index.ts:261` |
| Permission modes | `default`, `acceptEdits`, `plan`, `auto` (a classifier), `dontAsk`, `bypassPermissions`. Deny rules hold in every mode. D `cc:permission-modes` | The approval policies, plus `approvals_reviewer = auto_review` (an LLM decides). S `codex:protocol/src/config_types.rs:159-172` | `--auto` is client-side and answers every ask "once"; `deny` still holds. Plan mode denies edits but **not bash**. S `oc:opencode/src/cli/cmd/run.ts:801-822`, `oc:opencode/src/agent/agent.ts:156-181` |
| Process limits | Subagent depth 3 and 20 at once. No total limit, and no limit on background shells (I). D `cc:sub-agents` | `agents.max_depth` 1; 6 threads; 64 exec sessions; no rlimits. S `codex:config/src/config_toml.rs:682-693`, `codex:core/src/unified_exec/mod.rs:73` | `subagent_depth` 1. Background subagents are experimental. Max steps is enforced only by the prompt. S `oc:opencode/src/tool/task.ts:97-117`, `oc:opencode/src/session/prompt.ts:1178-1285` |
| Managed lock | `managed-settings.json`, MDM or server. Managed-only keys include `allowManagedPermissionRulesOnly`, `allowManagedHooksOnly` and `allowManagedDomainsOnly`. A local admin can still edit it. D `cc:managed-settings` | `requirements.toml`, cloud or MDM: `allowed_approval_policies`, `allowed_sandbox_modes`, `[rules]` (no `allow`), `allow_managed_hooks_only`. S `codex:config/src/config_requirements.rs:899-935` | `/etc/opencode`, `/Library/Application Support/opencode` or MDM. **`OPENCODE_PERMISSION` and per-agent rules are applied after it**, so managed is not final. S `oc:opencode/src/config/config.ts:530-565`, `oc:opencode/src/agent/agent.ts:293` |
| Its own config, against the agent | Protected paths prompt, but are **written without asking in `bypassPermissions`**. Sandboxed shell commands cannot write them. Settings hot-reload. D `cc:permission-modes`, `cc:sandboxing`, `cc:settings` | `.codex`, `.git` and `.agents` are read-only inside writable roots. **A trusted project's `.codex/config.toml` can loosen the sandbox and approvals.** S `codex:protocol/src/permissions.rs:22-69, 1600-1641`, `codex:config/src/loader/mod.rs:60-76` | **Unprotected.** The agent can edit `opencode.json` or write `.opencode/plugin/*.ts`, which loads with full authority next time. S (absence); I |

### 1.2 What each cannot enforce at all

**Claude Code** (D `cc:sandboxing`, `cc:permissions`, `cc:hooks`):

- OS confinement of its own file tools, WebFetch, hooks, MCP servers or mods. Its Read and Edit
  denies are in-process checks, "best-effort" for Grep and Glob.
- A shell deny rule as a boundary. Per-domain rules for traffic that is not proxy-aware: that
  traffic is blocked outright.
- Unix-socket paths and local binding on Linux.
- A fail-closed hook, unless the hook sets `onFailure: "block"`. A cap on total subagents or
  background shells.
- **Mods** (in-process plugin code) answering `tool.check` **override deny rules and `PreToolUse`
  blocks** unless managed settings, a managed MCP file or a Team/Enterprise login is present. D
  `cc:permissions` "Precedence among rules, hooks, and mods"

**Codex** (S, citations above):

- No sandbox for MCP servers, hooks or `notify`. No argument or path rules for commands, and none
  for edit targets. No deny list for built-in tools.
- An execpolicy `allow` drops the sandbox.
- Hooks fail open, and `write_stdin` (typing into a running command) is not hooked.
- A trusted project can loosen the sandbox unless requirements forbid it.
- No resource limits. Without its managed proxy, network is all or nothing.

**OpenCode** (S, citations above):

- No OS confinement of the shell or MCP servers: files, network and processes are all open. No
  shell egress control.
- Shell rules are raw text. Its `external_directory` check for the shell is a 12-command heuristic,
  so `echo > /x`, `tee` and `python -c` slip past.
- No URL rules for `webfetch`. Managed config is not final. Its own config, agents and plugins are
  open to the agent.
- No hook timeout. No hard step limit. "Always" approvals are kept in memory only.

## 2. purlis's enforcement points, and which harness each covers

| Point | What it is | Claude Code | Codex | OpenCode |
|---|---|---|---|---|
| **Neutral sandbox policy** | One `Compiled` policy, compiled per harness. Three levels: Full, Floor (the default) and Lifted (a per-chat opt-out). Five denial classes hold at every level but Lifted: Vaults, Integrity, HumanPowers, RunnerInternals, LaterCode. V `core/sandbox.rs:4-10, 988-1012, 3714-3731` | compiled to CC's `sandbox` keys plus `permissions.deny` (each denied path written twice) | compiled to the wrap's profile | compiled to the wrap's profile |
| **OS wrap** | Seatbelt on macOS; Landlock, seccomp and a supervisor on Linux. Last match wins; denials come after grants. V `core/sandbox/seatbelt.rs:996-1001`, `crates/linux-sandbox/src/lib.rs:1-20` | Linux only (`core/sandbox/claude_wrap.rs`) | always | always |
| **Egress proxy** | A per-chat loopback HTTP CONNECT and SOCKS5 proxy. It decides on host and port: open, persona, allowed, ask or refused. No TLS interception. V `core/sandbox/egress.rs:4-63`, `core/sandbox/asks.rs:4-45` | set as `network.httpProxyPort`/`socksProxyPort` (CC ≥2.1.285). Covers **shell commands only** | the wrap lets out only the proxy's ports and the hook socket | same as Codex |
| **Hooks and guards** | `toolgate::verdict` on Bash (arms A to A10), a vault guard on Read/Grep, a state-folder guard on edits, a dispatch gate. "Guard rails, not guarantees". V `core/toolgate.rs:234-277`; D `cdocs/secrets.md:703-722` | all, run outside the sandbox | Bash only, run inside the wrap (unverified) | bash, read, grep, write, edit and task, run inside the wrap (unverified) |
| **App-side checks** | Dispatch decisions, `secret exec`, browser posture and reach, `skill_load`, MCP tools confined to the chat's workspace. V `core/dispatchdecision.rs:4-35`, `core/chattools.rs:20-26`, `core/browsertools.rs:4-19` | same | same | same |
| **Start-time refusals** | A chat whose sandbox can't be applied is refused (`failIfUnavailable`, `NotStarted`). Flags that would loosen it are refused. V `core/sandbox/claude.rs:488-491`, `core/sandbox/codex.rs:770-830` | `allowUnsandboxedCommands:false`, `failIfUnavailable:true` | `loosened_by` refuses `-s`, `--yolo`, `--add-dir`, `-a` and so on | `disarmed_by` refuses `--pure`, `OPENCODE_PURE` and `OPENCODE_CONFIG_CONTENT` in a profile |

## 3. Per policy family

Verdicts:

- **Same**: the same outcome in all three harnesses, though the mechanism may differ.
- **Differs**: the outcome depends on the harness.
- **Gap**: no harness enforces it today.

### 3.1 Sandbox files: write roots, read denies, cross-workspace reads

**Writes and denial classes: same outcome, different mechanism.**

- **Claude Code:** its sandbox's `denyWrite`/`denyRead` for shell commands, plus `permissions.deny`
  `Read(...)`/`Edit(...)` for its file tools.
- **Codex and OpenCode:** the wrap.
- The write roots differ by what each harness needs to keep its own state. Codex writes only its
  sandbox-only `CODEX_HOME` stores. OpenCode writes only its sessions database, log and storage.

V `core/sandbox/claude.rs:7-45, 460-505`; V `core/sandbox/codex.rs:25-46`; V
`core/sandbox/opencode.rs:12-45`.

**Differs.**

- **Claude Code's file tools** are held by an in-process check, not by the OS. Its docs call Read
  denies "best-effort" for Grep and Glob (D `cc:permissions`). purlis backs them with its
  `pretooluse-read` guard (V `core/hookreg.rs`).
- **The keychain:** Claude Code's settings cannot deny the keychain service, so on macOS the
  Vaults class rests on the keyring item's own access rule (ruling V90a). The wrap denies the
  keychain for Codex and OpenCode. V `core/sandbox/claude.rs:18-24`

**Gap: cross-workspace reads.**

- No harness limits reads across workspaces at the OS level today. The wrap's base is
  `(allow file-read*)`, and the CC settings read everything outside the denial classes. Only
  purlis's own MCP tools are confined to the chat's workspace. V
  `core/sandbox/seatbelt.rs:996-1001`, `core/sandbox/confined.rs:8`, `core/chattools.rs:20-26`
- **This gap is purlis's, not the harnesses'.** It can be closed the same way in all three (I):
  - Claude Code's `denyRead`/`allowRead` covers shell commands, and `Read(...)` denies cover its
    tools. Claude Code also has `blockReadsOutsideWorkingDirectories`.
  - The wrap can deny reads by path on both OSes.
  - Codex's own `deny` read entries are not needed while Codex's sandbox is off.

### 3.2 Network

**Same.** Every chat's commands go through purlis's proxy, and an unlisted host is held with a live
ask in the window. V `core/sandbox/egress.rs:4-63`, `core/sandbox/asks.rs:4-45`

**Differs.** Claude Code's own process is not behind the proxy on macOS (I from
`core/sandbox/claude.rs:12-14` against `claude_wrap.rs:20-22`). That covers:

- its model traffic;
- WebFetch and WebSearch, which purlis **denies at Full** and allows at Floor;
- its hooks and every MCP server, purlis's or not, which have open network (D `cc:sandboxing`).

V `core/sandbox/claude.rs:120-129`

**Gap (Claude Code).** A non-purlis MCP server or hook in a Claude Code chat has unrestricted
network. The hold on it is ADR 0124 phase 3, not built. D `docs/adr/0124-…md:305-333, 624`

### 3.3 Ports: local binding and unix sockets

**Same grants** in all three: Port, Listen, Socket and Service. V `core/sandbox/grant.rs:67-81`

**Differs.**

- **Listening.** A Claude Code chat cannot make a socket, so its server dials in through `purlis
  listen` to purlis's door. Codex and OpenCode serve on a unix socket behind the door. V
  `core/sandbox/door.rs:4-45`
- **Claude Code on Linux.** `allowUnixSockets` is ignored (D `cc:settings-reference`), and "no door
  opens for a Claude Code chat" on the Linux wrap. V `core/sandbox/claude_wrap.rs:44-45`. Whether
  a Listen grant works there was not checked (I).
- **An administrator.** A managed `allowLocalBinding: true` outranks purlis's `false`. V
  `core/sandbox/claude.rs:830-850`

### 3.4 Processes

**Differs.**

- **Codex and OpenCode, in the wrap.** Exec and fork are allowed, and signals reach only the same
  sandbox. Floor denies `launchctl`, `osascript`, `open`, `crontab`, `at`, `batch`, Apple events
  and `lsopen`. V `core/sandbox/seatbelt.rs:880-899, 996-1001`
- **Claude Code.** purlis writes no process rules of its own and relies on Claude Code's sandbox
  (I). Claude Code turns Apple events off unless `allowAppleEvents` is set (D `cc:sandboxing`).

**Gap.** No harness limits how many background processes or subagents a chat starts beyond its own
built-in caps. purlis observes Claude Code's background work (`posttooluse-ownwork`) and has no
equivalent for Codex or OpenCode. V `core/hookreg.rs:150-158`

### 3.5 Guards

**Differs. This family has the most gaps.**

| Guard | Claude Code | Codex | OpenCode |
|---|---|---|---|
| Bash guard (`toolgate`, all arms) | yes, outside the sandbox | yes, inside the wrap. Inert until the TUI trusts the hooks; untrusted hooks don't run and nothing says so (V `core/harness/codex.rs:180-185`). **`write_stdin` is not hooked** (S, above) | yes, through the shim. Another plugin in the same JS realm can get round it (V `core/opencode.rs:62-72`) |
| Vault guard on Read/Grep | yes | **no** | yes |
| State-folder guard on edits | yes | **no** (`apply_patch` is not reported, V `core/touching.rs:32,75`) | yes |
| Config edit → proposal; skill edit → draft | yes | no | no (V `core/toolhooks.rs:638-646`) |
| A hook that times out | **the tool runs.** purlis does not set Claude Code's `onFailure: "block"` (V: absent from `core/` and the plugin) | **the tool runs** | the shim refuses |

D `cdocs/hooks.md:978-983`. The hard line behind every guard is the sandbox's denial classes,
which are the same everywhere. A guard is a rail against mistakes (D `cdocs/secrets.md:703-722`).

### 3.6 Asks

**Same rule:** only a person answers an ask; a chat can raise one, never answer it. A timeout never
allows or denies. V `core/harness/asks.rs:4-33`

**Differs: how an ask is heard, and whether one is made at all.**

- **Claude Code:** `PermissionRequest`, armed on `--settings`, plus `Notification`.
  `PermissionDenied` reports the harness's own refusals. V `core/harness/claude.rs:1393-1415`
- **Codex:**
  - A sandboxed chat runs `approval_policy = granular`, with sandbox approvals and permission
    requests off, so it **rejects what it would ask**. V `core/sandbox/codex.rs:775-795`
  - There are no command-pattern permissions, so a `guard ask` rule has nowhere to go. V
    `core/harness/codex.rs:368-381`
  - There is no Notification hook.
- **OpenCode:** the `permission.asked` event, answered with `client.permission.reply`. Its own
  `permission.ask` plugin hook is never called (S). **`ask_operator` cannot ask** (1.18.33 offers
  no MCP form). V `app/src-tauri/plugin/opencode/purlis.ts:337-362`; D `cdocs/harnesses.md:345-346`

### 3.7 Dispatch

**Same decision.** The app decides from its own record: grants, locks, attendance and helper
limits. V `core/dispatchdecision.rs:4-35`, `core/dispatchunattended.rs:215-300`

**Differs at the edges.**

| | Claude Code | Codex | OpenCode |
|---|---|---|---|
| A sub-agent may not dispatch | Bash guard and tool hook | Bash guard only | **none** |
| Attendance and permission mode read from | how purlis started the chat, and the armed hook's `permission_mode` | how purlis started the chat | how purlis started the chat |
| Flags that bypass, refused in a dispatch profile | `--dangerously-skip-permissions`, `--permission-mode bypassPermissions` | `--yolo`, `--full-auto`, `-a never`, and others | none listed (I) |

V `core/attendance.rs:4-31`, `core/dispatchprofiles.rs:23-30`; D `cdocs/hooks.md:713-747`.

### 3.8 Vaults

**Same.**

- The Vaults class denies direct reads.
- `purlis secret exec` runs the command in an app-built sandbox, with the chat's denials and its
  proxy, and masks the output.
- Identity variables (`OP_*`, `VAULT_TOKEN`) are stripped.
- A browser fill is done app-side.

D `cdocs/secrets.md:19-56`; V `core/secretfill.rs:4-17`.

**Differs.** The keychain on Claude Code under macOS (§3.1).

**Gap.** No harness keeps its own sign-in from the commands it runs. V
`core/harness_declaration/{claude,codex,opencode}.toml` (`kept_from_commands`)

### 3.9 Browser

**Same.** purlis's `browser_*` MCP tools carry the posture, a per-browser reach proxy, the holding
folder and app-side login fill. Each harness's own browser is off:

- **Claude Code:** `mcp__claude-in-chrome` is denied.
- **Codex:** `browser_use`, `computer_use` and `in_app_browser` are disabled.
- **OpenCode:** has no browser of its own.

V `core/browsertools.rs:4-19`, `core/sandbox/browser_reach.rs:4-33`, `core/sandbox/claude.rs:125-129`,
`core/sandbox/codex.rs:797-801`.

**Differs.** The MCP server runs outside any sandbox for Claude Code and inside the wrap for the
other two (D `docs/adr/0124-…md:617`). `cdocs/harnesses.md:318-319` says "outside" for Codex too,
which is probably stale for a sandboxed Codex chat (I).

### 3.10 Skills

**Same routing.** `skill_load` works on every harness. A chat cannot install a skill: LaterCode
denies `.claude/skills`, `.codex` and `.agents`, and `.opencode`. V `core/sandbox.rs:1103-1120`

**Differs: delivery.**

- **Claude Code:** a plugin through `--plugin-dir`.
- **Codex:** the `SessionStart` hook, plus `-c skills.config` to turn off a project-disabled skill.
  A running Codex chat sees a change only after a restart.
- **OpenCode:** `skills.paths`, plus a `permission.skill` deny.

The draft-on-edit path exists on Claude Code only. D `cdocs/harnesses.md:233-275`; V
`core/harness/codex.rs:78-127`, `core/opencode.rs:327-393`

### 3.11 Permission mode

**Differs, by nature.** The three harnesses' modes do not map onto one another (§1.1). What purlis
sets:

- **Claude Code:** nothing for a person's chat. A task starts in `auto` when its asking chat is in
  `auto` or has its prompts off. A grade's chat is pinned to `default`.
- **Codex:**
  - Unsandboxed: its own settings.
  - Sandboxed: `danger-full-access`, the granular-reject policy and `approvals_reviewer = "user"`.
    Flags that would loosen it are refused.
- **OpenCode:** nothing beyond refusing `--pure`.

V `core/harness/claude.rs:420-463`, `core/sandbox/codex.rs:770-830`, `core/opencode.rs:28-32`.

**Gap.** A policy key "permission mode" has no common meaning to compile to. It needs per-harness
semantics, or a definition in terms of asks: which actions ask whom (I).

### 3.12 Protecting the policy from the chat (Q4)

**Same, under a sandbox.** LaterCode denies writes, in each harness, to:

- **Claude Code:** `.claude/settings*.json`, `.claude/{commands,agents,skills,hooks,rules,plugins}`,
  `.mcp.json` and `~/.claude.json`, for both shell and `Edit(...)`.
- **Codex:** `.codex` and `.agents`.
- **OpenCode:** `opencode.json(c)`, `tui.json(c)`, `.opencode` and `~/.config/opencode`.
- **In every harness:** `charter.toml`, the workspace manifests and `[dispatch]`.

The session settings are out of the chat's reach:

- **Claude Code:** `--settings` lives in app data and is read back only if its digest matches.
- **Codex:** `-c` flags.
- **OpenCode:** `OPENCODE_CONFIG_CONTENT`.

V `core/sandbox.rs:1103-1120, 1823-1850`; D
`docs/adr/0067/2026-10-28-chat-settings-live-in-the-app-data-folder-and-the-state-folder-is-held-whole.md`.

These denies matter because two of the harnesses do not protect their own config:

- **Claude Code** writes protected paths without asking in `bypassPermissions` (D
  `cc:permission-modes`). A deny rule still holds in that mode, so purlis's `Edit(...)` deny keeps
  them.
- **OpenCode** protects nothing of its own (S).

**Gaps.**

- **A Lifted (unsandboxed) chat** has none of these OS denials, in any harness. Q4's "a chat can
  never change the policy that governs it" does not hold there today. V `core/sandbox.rs:3714-3731`;
  D `docs/adr/0067-…md:539`
- **A Claude Code mod** can override deny rules and `PreToolUse` blocks unless managed settings or
  a Team/Enterprise login is present (D `cc:permissions`). Whether a purlis chat can load a mod was
  not checked (I). This is to be verified.
- **Codex's** trusted-project loosening is not reachable. purlis's sandboxed Codex chat uses a
  `CODEX_HOME` of its own and marks folders untrusted. V `core/sandbox/codex.rs:25-56`

## 4. Known gaps for Q5

"Refuse the start?" names the candidate Q5 outcome. It is not a decision.

| # | Policy key family | Harness | Gap | What purlis does today | Refuse the start? |
|---|---|---|---|---|---|
| 1 | files: cross-workspace reads | all | no OS read limit | only purlis's MCP tools are confined | no: closable the same way in all three (§3.1) |
| 2 | files: Claude Code file tools | CC | an in-process check, not OS | `permissions.deny` plus the `pretooluse-read` guard | declare |
| 3 | vaults: keychain | CC (macOS) | settings can't deny the service | rests on the keyring item's access rule (V90a) | declare |
| 4 | vaults: harness sign-in | all | not kept from commands | nothing | declare |
| 5 | network: hooks and MCP servers | CC | outside the sandbox and proxy | phase 3 of ADR 0124 (not built) | a key that forbids non-purlis MCP servers would refuse |
| 6 | ports: listen | CC (Linux) | no door; `allowUnixSockets` ignored (whether Listen works there: unchecked) | nothing | verify first |
| 7 | ports: admin `allowLocalBinding` | CC | a managed setting outranks purlis | nothing | declare |
| 8 | processes: count of background work and subagents | all | only built-in caps | CC observed, none for the others | declare |
| 9 | guards: Read/Grep and Edit | Codex | no hook | the sandbox classes only | a guard key would refuse |
| 10 | guards: `write_stdin` | Codex | unhooked | nothing | declare |
| 11 | guards: trust | Codex | hooks inert until trusted, silently | nothing says so | a missing trust would refuse |
| 12 | guards: realm | OpenCode | another plugin can get round the shim | nothing; #1069 moves the hooks out of the wrap | declare |
| 13 | guards: run inside the wrap | Codex, OpenCode | reports are "unverified" | ADR 0124's matrix rates them so | declare |
| 14 | guards: hook timeout | CC, Codex | the tool runs | purlis bounds itself to 2 s; CC's `onFailure: "block"` is not set | CC: set it (I); Codex: declare |
| 15 | asks: command-pattern ask | Codex | no permissions file | the guard applies | a `guard ask` rule would refuse |
| 16 | asks: harness asks | Codex (sandboxed) | rejected, never asked | granular-reject | declare |
| 17 | asks: `ask_operator` | OpenCode | no MCP form | nothing | an ask-whom-on-question key would refuse |
| 18 | dispatch: sub-agent | OpenCode (none), Codex (Bash only) | no refusal | partial | a sub-agent dispatch key would refuse |
| 19 | dispatch: attendance and mode | Codex, OpenCode | read only from how purlis started the chat | nothing | declare |
| 20 | skills: config and skill edits | Codex, OpenCode | no proposal or draft | sandbox refusal only | declare |
| 21 | skills: live change | Codex | needs a restart | nothing | declare |
| 22 | permission mode | all | no common semantics | per-harness settings | needs defining (§3.11) |
| 23 | policy self-protection | all, when Lifted | no OS denials | nothing | the core of Q4 |
| 24 | policy self-protection: mods | CC | a mod overrides denies | not checked | verify first |

## Doc inconsistencies found

- `cdocs/hooks.md:732` says Codex hands purlis's tools no connection to the app.
  `cdocs/harnesses.md:332-335` says Codex is told the socket variables.
- `cdocs/harnesses.md:318-319` says the MCP server runs outside the sandbox for Codex. ADR 0124
  line 617 and `core/sandbox/claude_wrap.rs:17-19` put it inside the wrap.
- OpenCode's in-repo docs say `.env` reads are denied by default. Its source says `ask`
  (`oc:opencode/src/agent/agent.ts:130-135`).

## Prior work

ADR 0124 ("Who judges a chat, and how much", **proposed** 2026-11-12) already has policy tiers
(organisation, project requires, project recommends, person, chat). It also has a per-harness
matrix that rates each layer enforced, unverified or Can't tell, with a test per cell. D
`docs/adr/0124-…md:139-149, 605-626`. The gaps above extend that matrix from layers to Q307's
policy families.
