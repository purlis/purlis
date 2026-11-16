# R-772: what a chat reaches of the keychain

Whether anything inside a chat (a command the agent runs, a hook, a stdio MCP server) can get a
model key out of the system's keychain itself (T1a-72, #6764; decisions.md D6640-T3 X7 and item
7). The table has two halves:

- **The CI half** is what a test build can show: whether each caller reaches the keychain service
  at all, and whether it can read or write the test build's stub keyring. A test build never
  reaches the system's keychain, so it cannot show more.
  `the_keychain_service_reach_table_matches_the_pinned_table` measures every row for the system it
  runs on and fails on any cell that differs (Windows only reports one). Linux runs in `ci.yml`'s
  `rust` job, macOS in `studio-confinement-macos`, Windows in the nightly `windows` job.
- **The hand-run half** is the items themselves, on a signed build per system, run by a person.

Each project in both halves has a model key in your keys and no keyring vault: a non-empty your
keys index counts as a keyring vault (D6640-T3 item 7), so every chat's sandbox denies the
credential service. With an empty index the rule is as it was before.

## The CI half

A cell is **reachable** or **denied**. *hand-run* is a cell no test build can measure; the hand
run's half below holds it. The stub keyring is `<state>/keyring-stub.json` in the project, and
`keyring-stub.json` in purlis's config folder: a cell is *reachable* if either is.

- *Keychain service*: on macOS, the Seatbelt profile's rule for the security server's Mach service
  (`com.apple.SecurityServer`), and *reachable* where no profile holds the caller. On Linux,
  whether `org.freedesktop.secrets` answers on the session bus from inside the chat, with a real
  Secret Service unlocked on that bus. On Windows, whether `CredRead` answers.
- *Harness* `claude-code` on macOS: Claude Code's own sandbox holds the commands it runs, and its
  hooks and MCP servers run outside any profile, so those cells are *reachable*. Its commands'
  cells need the real Claude Code, so they are hand-run.
- At the basic sandbox (`floor`) on macOS, the profile starts from allowing everything, so the
  security server answers; each item's own rule (held to purlis's app, ruling V90a) is what keeps
  a caller out, and the hand run measures it.

<!-- ci-table:start -->
| OS | Level | Harness | Caller | Keychain service | Stub read | Stub write |
| --- | --- | --- | --- | --- | --- | --- |
| macos | full | opencode | command | denied | denied | denied |
| macos | full | opencode | hook | denied | denied | denied |
| macos | full | opencode | mcp | denied | denied | denied |
| macos | full | codex | command | denied | denied | denied |
| macos | full | codex | hook | denied | denied | denied |
| macos | full | codex | mcp | denied | denied | denied |
| macos | full | claude-code | command | hand-run | hand-run | hand-run |
| macos | full | claude-code | hook | reachable | reachable | reachable |
| macos | full | claude-code | mcp | reachable | reachable | reachable |
| macos | floor | opencode | command | reachable | denied | denied |
| macos | floor | opencode | hook | reachable | denied | denied |
| macos | floor | opencode | mcp | reachable | denied | denied |
| macos | floor | codex | command | reachable | denied | denied |
| macos | floor | codex | hook | reachable | denied | denied |
| macos | floor | codex | mcp | reachable | denied | denied |
| macos | floor | claude-code | command | hand-run | hand-run | hand-run |
| macos | floor | claude-code | hook | reachable | reachable | reachable |
| macos | floor | claude-code | mcp | reachable | reachable | reachable |
| linux | full | opencode | command | denied | denied | denied |
| linux | full | opencode | hook | denied | denied | denied |
| linux | full | opencode | mcp | denied | denied | denied |
| linux | full | codex | command | denied | denied | denied |
| linux | full | codex | hook | denied | denied | denied |
| linux | full | codex | mcp | denied | denied | denied |
| linux | full | claude-code | command | denied | denied | denied |
| linux | full | claude-code | hook | denied | denied | denied |
| linux | full | claude-code | mcp | denied | denied | denied |
| linux | floor | opencode | command | denied | denied | denied |
| linux | floor | opencode | hook | denied | denied | denied |
| linux | floor | opencode | mcp | denied | denied | denied |
| linux | floor | codex | command | denied | denied | denied |
| linux | floor | codex | hook | denied | denied | denied |
| linux | floor | codex | mcp | denied | denied | denied |
| linux | floor | claude-code | command | denied | denied | denied |
| linux | floor | claude-code | hook | denied | denied | denied |
| linux | floor | claude-code | mcp | denied | denied | denied |
| windows | none | any | command | reachable | reachable | reachable |
| windows | none | any | hook | reachable | reachable | reachable |
| windows | none | any | mcp | reachable | reachable | reachable |
<!-- ci-table:end -->

**Reachable cells, for the residual.** macOS at the basic sandbox: every caller reaches the
security server, and only the item's own rule stands between it and a key. macOS, Claude Code: its
hooks and MCP servers run outside any profile, so they reach the security server and, in a test
build, the stub keyring. Windows: purlis has no sandbox there, so every caller runs as the person.
The hand run says what each of those reaches of the items.

A session bus a person allowed a chat is not given to it while the credential service is denied
(D6640-T3 item 7): `with_one_your_keys_entry_a_session_bus_a_person_allowed_is_not_given_to_a_chat`
holds that, so no row here has a granted bus.

## The hand-run half

Run at the gate on a **signed build per system**, never a test build: a test build keeps its
keychain in the stub file and would read a stub line. Each cell is **denied**, **prompted** or
**readable**, with the error code it was mapped from. A keychain prompt is always *prompted*,
never denied (ruling V90a): *Always Allow* would open the item, so it is a residual.

### Steps

1. Install the signed build. In its window, add an Anthropic key under You › Models › Your keys,
   so the signed window creates the item `purlis/@your-keys` (account: the entry's id) and the
   mark's device key `purlis/@model-key-marks/<device>` (account `key`, T1a-10). On macOS the item's
   access list then names the signed app alone. Find the entry's id in
   `<config>/purlis/model-keys/index.json`; it is not secret.
2. On macOS, run it in a logged-in session, so a prompt the keychain draws is seen. On Linux, use
   a desktop session with the Secret Service unlocked.
3. In a project with no keyring vault, open a chat on each harness's profile (opencode, Codex,
   Claude Code) at each level (Settings › Sandbox: full, then basic) and run the probe as each
   caller, once for each item. The harness matters: on macOS a Claude Code chat's hooks and MCP
   servers run outside any profile, where an opencode or Codex chat's run inside its sandbox, so
   each harness has rows of its own:
   - **command**: ask the chat to run `sh <checkout>/crates/purlis-core/tests/keychain_reach/probe.sh fetch <service> <account>`
     (Windows: `powershell -NoProfile -File <checkout>\crates\purlis-core\tests\keychain_reach\probe.ps1 fetch <target>`);
   - **hook**: the same line as a `PreToolUse` hook command in the project's Claude Code
     settings, its output written to a file the hook names;
   - **mcp**: the same line with `--as mcp` (Windows `-As mcp`) as a stdio MCP server in the
     project's `.mcp.json`; its first answer carries the lines.
4. Write each outcome and its code into the table below, with the date and the build's version.

### Error codes and the outcome each maps to

| System | What came back | Outcome |
| --- | --- | --- |
| macOS | a value (exit 0) | readable |
| macOS | a prompt drawn, `errSecInteractionNotAllowed` (-25308), or `errSecUserCanceled` (-128) | prompted |
| macOS | `errSecAuthFailed` (-25293): a prompt drawn and answered *Deny* | prompted |
| macOS | -25308 on a headless Mac, where no prompt can draw | prompted |
| macOS | a sandbox refusal, or `errSecItemNotFound` (-25300) from inside the chat | denied |
| Linux | a value (exit 0) | readable |
| Linux | an unlock prompt drawn | prompted |
| Linux | exit 1 with nothing: no item found, or no Secret Service reachable | denied |
| Windows | a value (`CredRead` succeeds) | readable |
| Windows | `ERROR_NOT_FOUND` (1168), or any other error | denied |

Linux has no per-application rule, and a chat outside purlis's wrap runs as the person, so a
Linux cell may be *readable*; the table says so where it is.

**On Linux, a harness purlis doesn't wrap refuses to start sandboxed once a model key exists**
(W132): its own sandbox can't keep a command off the session bus, where the Secret Service gives
any process of the session every model key with no prompt, so the chat is refused rather than
started. Today purlis wraps every harness there (Codex and opencode whole, Claude Code with its
own sandbox off, #2037), so no chat is refused for it; a harness added without the wrap would be.
This is a residual for the operator beside the cells below.

**A chat already running when the first model key is added keeps its sandbox until it restarts**
(R-775, T1a-78): it was settled without the keyring rule, so it reaches the credential service as
its cells above read with no model key. purlis owes it a restart at its next pause between turns,
never mid-turn, and shows W140 with *Restart now*; a chat taken back at open is judged the same
way. The time between the key being added and that pause stays open: a residual for the operator
beside the cells below.

### The items

Your keys' item is `purlis/@your-keys` (account: the entry's id). The mark's device key is
`purlis/@model-key-marks/<device>` (account `key`). Windows names each as its target,
`<account>.<service>` as the `keyring` crate writes it.

<!-- hand-table:start -->
| OS | Level | Harness | Caller | Item | Outcome | Error code | Run on (date, build) |
| --- | --- | --- | --- | --- | --- | --- | --- |
| macos | full | opencode | command | your-keys | to record | | |
| macos | full | opencode | command | mark-key | to record | | |
| macos | full | opencode | hook | your-keys | to record | | |
| macos | full | opencode | hook | mark-key | to record | | |
| macos | full | opencode | mcp | your-keys | to record | | |
| macos | full | opencode | mcp | mark-key | to record | | |
| macos | full | codex | command | your-keys | to record | | |
| macos | full | codex | command | mark-key | to record | | |
| macos | full | codex | hook | your-keys | to record | | |
| macos | full | codex | hook | mark-key | to record | | |
| macos | full | codex | mcp | your-keys | to record | | |
| macos | full | codex | mcp | mark-key | to record | | |
| macos | full | claude-code | command | your-keys | to record | | |
| macos | full | claude-code | command | mark-key | to record | | |
| macos | full | claude-code | hook | your-keys | to record | | |
| macos | full | claude-code | hook | mark-key | to record | | |
| macos | full | claude-code | mcp | your-keys | to record | | |
| macos | full | claude-code | mcp | mark-key | to record | | |
| macos | floor | opencode | command | your-keys | to record | | |
| macos | floor | opencode | command | mark-key | to record | | |
| macos | floor | opencode | hook | your-keys | to record | | |
| macos | floor | opencode | hook | mark-key | to record | | |
| macos | floor | opencode | mcp | your-keys | to record | | |
| macos | floor | opencode | mcp | mark-key | to record | | |
| macos | floor | codex | command | your-keys | to record | | |
| macos | floor | codex | command | mark-key | to record | | |
| macos | floor | codex | hook | your-keys | to record | | |
| macos | floor | codex | hook | mark-key | to record | | |
| macos | floor | codex | mcp | your-keys | to record | | |
| macos | floor | codex | mcp | mark-key | to record | | |
| macos | floor | claude-code | command | your-keys | to record | | |
| macos | floor | claude-code | command | mark-key | to record | | |
| macos | floor | claude-code | hook | your-keys | to record | | |
| macos | floor | claude-code | hook | mark-key | to record | | |
| macos | floor | claude-code | mcp | your-keys | to record | | |
| macos | floor | claude-code | mcp | mark-key | to record | | |
| linux | full | opencode | command | your-keys | to record | | |
| linux | full | opencode | command | mark-key | to record | | |
| linux | full | opencode | hook | your-keys | to record | | |
| linux | full | opencode | hook | mark-key | to record | | |
| linux | full | opencode | mcp | your-keys | to record | | |
| linux | full | opencode | mcp | mark-key | to record | | |
| linux | full | codex | command | your-keys | to record | | |
| linux | full | codex | command | mark-key | to record | | |
| linux | full | codex | hook | your-keys | to record | | |
| linux | full | codex | hook | mark-key | to record | | |
| linux | full | codex | mcp | your-keys | to record | | |
| linux | full | codex | mcp | mark-key | to record | | |
| linux | full | claude-code | command | your-keys | to record | | |
| linux | full | claude-code | command | mark-key | to record | | |
| linux | full | claude-code | hook | your-keys | to record | | |
| linux | full | claude-code | hook | mark-key | to record | | |
| linux | full | claude-code | mcp | your-keys | to record | | |
| linux | full | claude-code | mcp | mark-key | to record | | |
| linux | floor | opencode | command | your-keys | to record | | |
| linux | floor | opencode | command | mark-key | to record | | |
| linux | floor | opencode | hook | your-keys | to record | | |
| linux | floor | opencode | hook | mark-key | to record | | |
| linux | floor | opencode | mcp | your-keys | to record | | |
| linux | floor | opencode | mcp | mark-key | to record | | |
| linux | floor | codex | command | your-keys | to record | | |
| linux | floor | codex | command | mark-key | to record | | |
| linux | floor | codex | hook | your-keys | to record | | |
| linux | floor | codex | hook | mark-key | to record | | |
| linux | floor | codex | mcp | your-keys | to record | | |
| linux | floor | codex | mcp | mark-key | to record | | |
| linux | floor | claude-code | command | your-keys | to record | | |
| linux | floor | claude-code | command | mark-key | to record | | |
| linux | floor | claude-code | hook | your-keys | to record | | |
| linux | floor | claude-code | hook | mark-key | to record | | |
| linux | floor | claude-code | mcp | your-keys | to record | | |
| linux | floor | claude-code | mcp | mark-key | to record | | |
| windows | none | any | command | your-keys | to record | | |
| windows | none | any | command | mark-key | to record | | |
| windows | none | any | hook | your-keys | to record | | |
| windows | none | any | hook | mark-key | to record | | |
| windows | none | any | mcp | your-keys | to record | | |
| windows | none | any | mcp | mark-key | to record | | |
<!-- hand-table:end -->

Every *prompted* and *readable* cell, once recorded, is the residual T1a-17 posts on the epic at
set (i)'s checkpoint, for the operator's acceptance (recorded in `decisions.md`) before T1a-48
writes it into ADR 0067's amendment.
