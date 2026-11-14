# Integration surfaces

This page lists each surface through which something outside purlis's core reaches it: an
extension, a pinned page, a chat's harness, a CLI reader, the forge, another chat. For each
surface it answers four questions:

- **Versioning:** what carries the version, who checks it, and what a mismatch answers.
- **Approval:** what grants access, where the grant is stored and in which tier, what
  invalidates it, and what the person sees when asked again.
- **Recovery:** what happens on a stale grant, a changed fingerprint or origin, an unknown
  version or request, a crashed program or an offline forge, and whether it recovers without a
  restart.
- **Tests:** which tests prove the behaviour.

The page then lists the gaps and names the seams where one shared piece would remove the most
inconsistency. It is the input to an architecture pass on integration load, so it describes and
does not propose.

It was written from the code at `041865944` (2026-11-13), not from the ADRs. The ADRs give
intent; where code and ADR disagree, the code is recorded and the disagreement is a gap. Every
`path:line` was read at that commit. A line drifts as the code moves; the function or constant
named beside it is the stable anchor.

**UNTESTED** means a search of `crates/*/tests`, the `#[cfg(test)]` modules, `*_tests.rs`,
`app/src/**/*.test.ts(x)` and `app/e2e/` found no test of that behaviour. It does not mean the
behaviour is wrong.

Abbreviations in paths:

| Short  | Path                          |
| ------ | ----------------------------- |
| `core` | `crates/purlis-core/src`      |
| `cli`  | `crates/purlis-cli/src`       |
| `tauri` | `app/src-tauri/src`          |
| `ui`   | `app/src`                     |
| `hw`   | `crates/purlis-core/src/hookwire.rs` |
| `pf`   | `docs/plane-format.md`        |

The tiers are those of ADR 0069, as [`plane-format.md`](plane-format.md) records them: Plane,
Clone state, Machine (syncable or device-bound) and Keyring.

## The surfaces at a glance

| #   | Surface | Who reaches the core | Version carrier | Grant store and tier |
| --- | --- | --- | --- | --- |
| 1   | Extension protocol | an extension's program, one process per question | manifest `version` (1 to 8); `"charter"` on every line | `<config>/extensions.json`, Machine, device-bound |
| 2   | Artifact page API | a pinned page's script, over a `MessageChannel` port | `"api": 1` in the page; `<id>@<N>` per query | `<config>/artifact-approvals.json`, Machine, device-bound |
| 3   | Tool catalogue and MCP twins | a chat over MCP stdio; a page; `purlis queries` | `since` per query (printed only); MCP protocol by `rmcp` | the harness's own allow list, made at each chat start |
| 4   | Regenerate | a pin's command, run in a sandbox by the session host | none (hash domain tags only) | `<config>/artifact-regenerate-approvals.json`, Machine, device-bound |
| 5   | Live subscriptions | a page subscribing to a query | the subscription's `<id>@<N>` | as 2; subscriptions are in memory |
| 6   | Hook lines | a harness's hooks, over `app/hooks.sock` | line shape (no field); framed `VERSION = 1`; spool `v: 1` | per-chat token, in memory; spool keys, Clone state, transient |
| 7   | Session protocol | the window, the CLI or another host, over `purlisd.sock` | a hello naming majors; host speaks N and N-1 | credentials in the machine store |
| 8   | ACP (level 3) | an agent over its stdio | ACP V1 at `initialize` | no grant; nothing offered beyond the session |
| 9   | Harness declarations and plugin | a committed declaration; the plugin purlis installs into a harness | none (`tested` is free text); plugin by content diff | `.charter/harness-declarations-approved.json`, Clone state |
| 10  | CLI machine output and exit codes | chats, the plugin build, CI | per command: `api`, `schema`, or nothing | none |
| 11  | The forge | GitHub or GitLab, through the native transport or `gh`/`glab` | REST `2022-11-28`; `forge-accounts.json` `version` | token in the Keyring; account list Machine, device-bound |
| 12  | Dispatch | another chat, through the CLI or MCP, over the hook socket | queue `v: 1`; record and away `VERSION` | per pair: chat memory, `app/sandbox.json`, committed `[dispatch.grants]` |
| 13  | Others | project file, themes, skill grades, links, updater, browser engine | see [Other surfaces](#13-other-surfaces) | — |

## 1. Extension protocol

ADR 0053 and its amendments; [`extension-api.md`](extension-api.md). The newest protocol is
`PROTOCOL = 8` (`core/executor.rs:161`). Queries need 7, artifact templates need 8.

| Facet | Behaviour | Evidence | Test |
| --- | --- | --- | --- |
| Versioning | The manifest's top-level `version` is the protocol, checked each time the manifest is parsed. There is no handshake. Each request line carries `"charter": <manifest's protocol>`, and the program also gets `PURLIS_PROTOCOL` | `core/extension.rs:1199-1222`; `core/executor.rs:2643`, `:2604` | `core/extension/tests.rs:2124`; `core/executor/tests.rs:2677`. `PURLIS_PROTOCOL`: UNTESTED |
| Versioning | **Older client** (1 to 7) is accepted and asked in its own protocol | `core/extension.rs:1209` | `core/extension/tests.rs:2124` |
| Versioning | **Newer client** (above 8), and version 0, refuse the whole manifest: "is version N, and this purlis reads versions 1 to 8" | `core/extension.rs:1212-1218` | `core/extension/tests.rs:222`, `:2182` |
| Versioning | **Missing** version: refused, "says no version" | `core/extension.rs:1220` | `core/extension/tests.rs:241` |
| Versioning | **Unknown or garbage** (`"8"`, `7.5`, negative) reads as missing, because `as_u64` gives `None`, so it gets the "no version" sentence | `core/extension.rs:1208`, `:1220` | UNTESTED |
| Versioning | Feature gates: each capability has a `since()` (queries 7, artifacts 8, check-reporter 6, grader 4, studio 3). A manifest asking a capability above its own `version` is refused | `core/extension/capability.rs:226-250`; `core/extension.rs:1227-1237` | `crates/extension-probe/tests/queries_through_the_executor.rs:309`; `core/extension/artifacts/tests.rs:104`; `core/extension/tests.rs:2229` |
| Versioning | An answer's `"charter"` must equal the protocol asked in, and its keys are a closed set | `core/executor.rs:3039-3061`, `:3116-3144` | `core/executor/tests.rs:1115`, `:1121`, `:2208` |
| Approval | The grant: path, approved SHA-256 fingerprint, source, on/off and `reads-prompts`, in `<config>/extensions.json`, **Machine, device-bound** | `core/extension.rs:203`, `:845-861`, `:2350`; `pf:8725` | `core/extension/tests.rs:1385` |
| Approval | The fingerprint covers the whole folder, plus the manifest's protocol when there is a program. A built-in is approved by its place in the bundle; a themes-only extension needs none | `core/extension.rs:1892-1922`, `:1017-1043` | `core/extension/tests.rs:2364` |
| Approval | **Invalidated by** any byte in the folder (an undeclared file too), or a move. **Not** by writes into its declared state folder, nor a purlis upgrade | `core/extension.rs:1017-1043` | `core/executor/tests.rs:272`; `core/extension/tests.rs:1385`, `:2364` |
| Approval | An unreadable record approves nothing | `core/extension.rs:2448-2487` | `core/extension/tests.rs:1220`, `:1236`, `:1639` |
| Approval | **On re-ask** the list row reads "changed since you approved it — contributing nothing", with Review. The dialog title is "“X” has changed since you approved it", with "Trust it anyway". A press is refused with "…has changed since you approved it … Open Extensions…" | `ui/Extensions.tsx:444-449`, `:372`; `ui/ApproveExtension.tsx:61-79`; `core/executor.rs:2377` | the "Trust it anyway" copy: UNTESTED |
| Recovery | Stale grant or changed fingerprint: every question re-reads the record and re-fingerprints the folder before it starts, so a change is seen with **no restart** | `core/executor.rs:2257-2318`, `:1148-1154` | `core/executor/tests.rs:230`, `:1382` |
| Recovery | Hung program: a 5 s deadline, then the whole process group is killed and its stderr tail quoted. A query answers `timeout` | `core/executor.rs:175`, `:1758` | `core/executor/tests.rs:401`, `:465`, `:1316`; `queries_through_the_executor.rs:283` |
| Recovery | Crashed program: its exit status or signal, plus the last stderr. There is **no restart, retry or backoff**: each question is a fresh process | `core/executor.rs:1773-1798` | `core/executor/tests.rs:531`, `:542`, `:1523` |
| Recovery | One question in flight per extension: a view, action or command is refused while one is busy; an event, studio question or query waits up to the deadline | `core/executor.rs:1384-1435`, `:1187-1195` | `core/executor/tests.rs:750`, `:2270` |
| Recovery | Unknown query or method: `needs-extension`, or refused before anything starts | `core/extension/queries.rs:372-378`; `core/executor.rs:1050` | `queries_through_the_executor.rs:212`; `core/executor/tests.rs:376`, `:1756`, `:2111` |
| Recovery | One failure, three answers to a page: a manifest that no longer parses is `needs-extension`, a changed one that parses is `not-approved`, a failed re-read is `failed` | `core/extension/queries.rs:371`, `:382`, `:384-388` | — |
| Tests | Frozen fixtures exist for protocols 4 and 5 only (`tests/fixtures/extension-api/v1.json`, `v2.json`), though `extension-api.md` says each protocol is frozen once recorded. Also UNTESTED: a version-8 extension answering a query under `"charter": 8`; a query answer with an unknown key, no `data`, or a non-text `cursor` | `docs/extension-api.md:34-46`; `core/executor.rs:157-160`, `:711-721`, `:1283` | UNTESTED |

## 2. Artifact page API

ADR 0122 and its amendments; [`artifact-links.md`](artifact-links.md).

**There is no TCP port.** "The port" is a `MessageChannel` port that purlis's injected script
gives the page as `window.purlis.port` (`tauri/artifacts.rs:113-150`). Each ask goes out as an image
GET to the page's own origin on the `artifact://` scheme and is attributed by the webview's label
(`tauri/artifacts.rs:306-331`). The CSP is `connect-src 'none'` (`core/artifact/serve.rs:234-246`).
What can go away is the app's in-memory `Artifacts` state (`tauri/artifacts.rs:222-228`), not a
socket.

| Facet | Behaviour | Evidence | Test |
| --- | --- | --- | --- |
| Versioning | The page declares `"api": N` in `<script type="application/json" id="purlis">`; this build answers `API = 1`. Each query is declared `<id>@<N>`, with N a canonical integer in `1..=API` | `core/catalogue.rs:29`, `:32`; `core/artifact/data.rs:58-74`; `core/artifact/calls.rs:60-69` | `core/catalogue/tests.rs:16`; `core/artifact/calls.rs:271` |
| Versioning | The core parses the declaration once per snapshot; the app reads it at open, reload and history | `tauri/artifacts.rs:974-980`, `:1023`, `:450`; `tauri/artifacts/history.rs:204` | — |
| Versioning | **Newer or unknown** `api`, an **unknown query**, or `@N` above `API`: the whole declaration is an `Err`. The app turns it into `None` (`.ok().flatten()`), so every read and call answers `not-approved`, **and the window says nothing**. `ArtifactOpened` has no field for the reason; only `purlis artifact check` (and its MCP twin) says why | `core/artifact/data.rs:67-74`; `core/artifact/calls.rs:65-79`; `tauri/artifacts.rs:977-979`, `:1094-1101`; `core/artifact/check.rs:49-55` | the refusal: `core/artifact/data.rs:330`; check: `core/artifact/tools/tests.rs:107`, `:149`. The silence in the window: UNTESTED |
| Versioning | **Older page:** every N in `1..=API` keeps answering, all mapped to the same `Query`. `@N` selects no behaviour (`Call::query` ignores it), and there is no way to retire one | `core/catalogue.rs:31`; `core/artifact/calls.rs:46-48`, `:194` | `core/artifact/calls.rs:294` (trivial while `API = 1`) |
| Versioning | A renamed or deprecated **id** still answers and the bar warns. No query sets `replaces` or `deprecated` today | `core/catalogue.rs:356-371`, `:282-344` | `core/catalogue/tests.rs:110`, `:201` (the warning paths: UNTESTED) |
| Versioning | An extension query `<ext>.<name>@N` is bounded by **purlis's** `API`, then `@N` is dropped and the bare id asked | `core/artifact/calls.rs:65-73`, `:203-217` | `core/artifact/calls.rs:283`, `:361` |
| Versioning | `window.purlis.api` is the literal `1` in the port script, not `catalogue::API`; the author kit says it is "the API version purlis answers this page in" | `tauri/artifacts.rs:149`; `core/artifact/kit.rs:111-112` | UNTESTED |
| Versioning | Offers and links carry no version: an offer's `does` is one of a closed set (`OFFERABLE`); a `purlis://` link is one of four shapes; anything else is refused with a sentence | `core/artifact/offers.rs:42-68`; `ui/pageOffers.ts:51-57`; `core/artifact/places.rs:52` | `core/artifact/offers.rs:135`; `app/e2e/specs/artifact-offers.e2e.ts:170`; `core/artifact/places.rs:217`, `:254` |
| Approval | "Approve script" in purlis's bar approves the pin id and the fingerprint **this webview was served**, never ones the window passes. A page with no script asks nothing | `tauri/artifacts.rs:1398-1432`; `core/artifact/approval.rs:264-266` | `tauri/artifacts.rs:1814`; `ui/ArtifactTab.test.tsx:470`; `core/artifact/approval.rs:514` |
| Approval | Fingerprint: SHA-256 over every path and byte of the folder except its data files, so the declaration is in it. Key: canonical workspace path plus the folder's `origin_host` (16 bytes of a SHA-256) | `core/artifact/serve.rs:189-198`, `:217-227`; `core/artifact/approval.rs:106-111` | `core/artifact/approval.rs:566`, `:621` |
| Approval | Store: `<config>/artifact-approvals.json`, **Machine, device-bound**, `0600`, replaced whole. Plane-scoped queries need a second `plane` grant, which a new approval drops | `core/artifact/approval.rs:5`, `:35`, `:331-376`, `:426-442`; `pf:8721` | `core/artifact/approval.rs:526`, `:860`; `core/artifact/calls.rs:422` |
| Approval | **Invalidated by** a template change (`Changed`); a change of pin folder, command or cwd, or an old 12-digit id (`PinChanged`); unpinning here; a fork or clone elsewhere. A workspace **rename carries** it (`moved`) | `core/artifact/approval.rs:233-241`, `:276-277`, `:393-421`; `core/artifact/pins.rs:103-119`; `core/artifact/mod.rs:428-442`; `core/wscmd/rename.rs:1298` | `core/artifact/approval.rs:566`, `:585`, `:603`, `:789`, `:815`, `:851` |
| Approval | **Origin:** an approved page is served from an origin hashed from a random `storage`, the pin and the fingerprint, so any re-approval gives a new, empty origin | `core/artifact/approval.rs:140-185`, `:312-313` | `core/artifact/approval.rs:663`, `:714`; `tauri/artifacts.rs:2328`; `app/e2e/specs/artifact-isolation.e2e.ts:359` |
| Approval | Re-gated at **every ask**: the pin still the one served, the standing still `Approved`. An approval in one window reaches every window and each reloads | `tauri/artifacts.rs:643-655`, `:1422-1430`; `tauri/artifacts/calls.rs:229-243` | `tauri/artifacts.rs:2091`; `ui/ArtifactTab.test.tsx:530` |
| Approval | Unreadable record (missing, malformed, a link, over 1 MiB) reads as no approval. Its `version` field is written and never read | `core/artifact/approval.rs:208-228`, `:432` | `core/artifact/approval.rs:639` |
| Approval | Script runs only on macOS and Linux; elsewhere the standing is `NotHere` | `core/artifact/approval.rs:45-55`, `:267-269` | `core/artifact/approval.rs:931`, `:943`; `ui/ArtifactTab.test.tsx:496` |
| Approval | **On re-ask** the bar shows a sentence in a `<span>`, not a `Notice`: "Has script, shown without it until you approve it", "Its template changed since you approved it…", "`<who>` changed its pin since you approved it…", plus "Approve script" | `ui/ArtifactTab.tsx:232-282`; `tauri/artifacts.rs:1023-1025` | `ui/ArtifactTab.test.tsx:438`, `:449`, `:486` |
| Approval | Links: "Always open" for an `https:` host is `<config>/artifact-link-hosts.json`, **Machine, device-bound**; a press is followed only with the load's one-time token | `pf:8724`; `tauri/artifact_links.rs:14`, `:68-86`, `:238-256` | `tauri/artifact_links.rs:699`, `:712`, `:743`, `:822` |
| Recovery | Grant goes stale while open: a plane-watch change reloads the standing and a template change reopens the page script-free; asks in flight answer `not-approved`. **No restart** | `tauri/artifacts.rs:427-468`, `:1488-1549`; `ui/ArtifactTab.tsx:539-555` | `tauri/artifacts.rs:2289`; `ui/ArtifactTab.test.tsx:545` |
| Recovery | Origin changed: reload navigates to the new origin; the old one's requests match nothing (404), and its offers and links are ignored | `tauri/artifacts.rs:311-312`, `:530-544`, `:1531-1539` | `tauri/artifacts.rs:1742`, `:2328` |
| Recovery | Page reloads: the port is revoked, old answers are never delivered, subscriptions are dropped, a new port opens | `tauri/artifacts.rs:362-368`, `:607-609` | `tauri/artifacts.rs:1978` |
| Recovery | **A malformed ask, one over 8 KiB, one before `open`, or one to a load not open gets no answer at all**, and the port script has no timeout, so the page's promise never settles. Asks of the wrong local shape do get `refused` | `tauri/artifacts.rs:245-288`, `:113-148`, `:128-131`; `tauri/artifacts/calls.rs:26`, `:30-45` | UNTESTED |
| Recovery | The plane no longer held: `failed` | `tauri/artifacts/calls.rs:196-198` | UNTESTED |
| Recovery | App restart: approvals persist on disk; a restored tab runs nothing until Open is pressed, which makes a new webview, load and port. purlis restores no subscription; the page's script subscribes again | `tauri/artifacts.rs:222-228`; `ui/ArtifactTab.tsx:49`, `:196-200` | `ui/ArtifactTab.test.tsx:165` |
| Recovery | Extension missing: `needs-extension`, and the bar warns; extension slow: `timeout` | `tauri/artifacts/queried.rs:61-66`; `core/artifact/calls.rs:152-161`; `core/extension/queries.rs:443` | `core/artifact/calls.rs:392` |
| Recovery | Forge offline for `purlis.forge_milestones`: `failed` with "purlis could not reach GitHub … It asks again in a minute". Only a **subscription** is asked again (section 5); a one-shot query is not | `core/catalogue/milestones.rs:182-185` | `core/catalogue/milestones/tests.rs:203` |

## 3. Tool catalogue and its MCP twins

One list of tools, `catalogue::every_tool()` (`core/catalogue.rs:36`), chains `chattools::TOOLS`,
the browser, theme-preview, skill and artifact tools, and the MCP server lists exactly that
(`cli/mcp.rs:81-97`, `:322-324`). `CATALOGUE` (`core/catalogue.rs:278`) holds 7 queries, each a
read-only MCP tool whose id is `purlis.<tool>` and whose parameters are the tool's schema
(`core/catalogue.rs:85-88`, `:102-104`). A chat asks by MCP `tools/call` and gets text; a page asks
through `catalogue::answer::ask` and gets the data envelope (`tauri/artifacts/calls.rs:199`);
`purlis queries` only lists (`cli/queries.rs:12-25`).

| Facet | Behaviour | Evidence | Test |
| --- | --- | --- | --- |
| Versioning | Each query has a `since` release string. It is only printed, in the plain and `--json` listings; nothing compares it with anything | `core/catalogue.rs:67`, `:112`; `cli/queries.rs:29-34` | `core/catalogue/tests.rs:138`; `crates/purlis-cli/tests/it/purlis_queries_lists_the_catalogue.rs:30`, `:66` |
| Versioning | **`since` is false for five queries:** `FIRST = "0.5.1"`, but `v0.5.1` has no `catalogue.rs`; the first release answering them is 0.5.2. `purlis.chats` spells `"0.5.2"` as a literal, `forge_milestones` uses a constant | `core/catalogue.rs:137`, `:140`, `:331`, `:342`; `git cat-file -e v0.5.1:crates/purlis-core/src/catalogue.rs` fails | UNTESTED; `tests/fixtures/queries/v1.json` does not pin `since` |
| Versioning | Released ids and result shapes are pinned by a fixture | `tests/fixtures/queries/v1.json` | `core/catalogue/tests.rs:16`, `:110`, `:166` |
| Versioning | An MCP tool carries no version. `serverInfo.version` is the crate version; the MCP protocol version is negotiated by `rmcp` 3.5. purlis reads it in one place: `ask_operator` answers `input_required` at 2026-07-28 or later (a string compare) and elicits before | `cli/mcp.rs:72-79`, `:824-840`; `crates/purlis-cli/Cargo.toml:42` | `crates/purlis-cli/tests/it/mcp_server.rs:155`, `:512`, `:639`, `:680`, `:697`. `serverInfo.version` and an unsupported protocol: UNTESTED |
| Versioning | **Unknown tool:** JSON-RPC `invalid_params`, "purlis has no tool …"; in a grade's chat, any tool outside `PRE_ALLOWED` too | `cli/mcp.rs:109-114`, `:177-182` | `mcp_server.rs:500`, `:1214` |
| Approval | **Claude Code:** the 18 `PRE_ALLOWED` tools, each named by hand, go into its `--settings` allow list as `mcp__purlis__<tool>`; `OWN_SCOPE_TOOLS` and the dispatch-grant tools are allowed beside them | `core/chattools.rs:229-251`, `:263-286`, `:594-604`; `core/harness/claude.rs:819-842` | `core/chattools/tests.rs:1232`, `:1262`; `core/harness/tests.rs:668`, `:906` |
| Approval | `forge_milestones` and `chats` are read-only queries deliberately **not** pre-allowed (operator rulings of 2026-11-11), so they ask through the harness's prompt | `core/harness/tests.rs:965-969`; `core/chattools.rs:1365-1367` | `core/harness/tests.rs:906` |
| Approval | **Codex and opencode:** only the dispatch-grant tools are pre-approved, so the 18 "pre-allowed" reads still prompt there. The operator's own Codex approvals are carried over and win | `core/chattools.rs:364-396`, `:403-420`, `:447-470` | `core/harness/codex/tests.rs:297`, `:324`; `core/harness/opencode/tests.rs:47` |
| Approval | Store: the allow list is made fresh at every chat start, passed inline or as `<app data>/chat-settings/<clone>/<chat>.<digest>.json`, **Machine, device-bound, transient**, swept after two minutes | `core/hoststart.rs:1310-1320`; `pf:8738` | in `hoststart` (not traced here) |
| Approval | The person's "always allow" is the harness's own store; purlis neither reads nor writes it. **What the person sees on re-ask is the harness's prompt, which purlis does not define** | `core/harness/claude.rs:787-792` | out of purlis |
| Recovery | A tool refusal is a tool result (`isError: true`); an unknown tool is a protocol error; the session goes on in both | `cli/mcp.rs:177-182`, `:733-747` | `mcp_server.rs:486`, `:500` |
| Recovery | **purlis's own MCP server crashing or restarting is undefined.** It is a stdio child of the harness, one per chat, holding nothing but `Waiting`; nothing restarts it or tells the chat. The tab Notice with Retry (`mcpfailed.rs`) covers only a persona's credentialed servers | `cli/mcp.rs:35-58`; `tauri/mcpfailed.rs:1-26` | UNTESTED |
| Recovery | purlis sends no `tools/list_changed`, so a new tool needs a new chat | `cli/mcp.rs:81-97` | UNTESTED |
| Recovery | A project turning read-only mid-session refuses writes at each call | `core/chattools.rs:1430-1436` | `mcp_server.rs:725`; `crates/purlis-cli/tests/it/the_mcp_server_on_a_read_only_project_changes_nothing.rs` |
| Recovery | `forge_milestones` over MCP: no login, offline, spent limit, 404 or 403 each answer one error text. Nothing retries, yet the chat is told "It asks again in a minute". A page asks the forge as the person, a chat as itself (`Caller::mcp`), so the twins can answer differently | `core/chattools.rs:1612-1625`; `core/catalogue/milestones.rs:10-13`, `:176-199`; `tauri/artifacts/calls.rs:215-220` | core `read`: `core/catalogue/milestones/tests.rs:203`; the MCP path: UNTESTED |
| Tests | The MCP list and the catalogue are held in step: every query is a read-only tool; every read-only tool is a query or in `NOT_QUERIES`; parameters match; the exact `tools/list` order; the `queries` tool equals `catalogue::listing()` | — | `core/catalogue/tests.rs:77`, `:90`, `:154`; `mcp_server.rs:155`, `:1249` |

## 4. Regenerate

A pin's command re-making its page's data, approved by a fingerprint of its code and run in a
sandbox.

| Facet | Behaviour | Evidence | Test |
| --- | --- | --- | --- |
| Versioning | No API version. Each hash has a fixed domain tag (`purlis-regenerate-run`, `purlis-regenerate-code`, both `"1"`). The record's `version: 1` is written and never read | `core/artifact/regenerate.rs:177`, `:287`, `:416-440`, `:495` | UNTESTED |
| Versioning | Regenerate writes the page's `data` list, so it inherits section 2: a refused declaration makes it refuse with "its page's declaration is refused: …" | `tauri/regenerate.rs:93-101` | UNTESTED |
| Approval | The code: the first non-flag word that is a file inside the plane is the script; its code folder is the nearest `SKILL.md` folder, else its own, and must sit strictly below a workspace root or in a skill's folder | `core/artifact/regenerate.rs:102-163`, `:210-268` | `core/artifact/regenerate/tests.rs:81`, `:98`, `:127-238`, `:746`, `:770` |
| Approval | The fingerprint: every path, kind, exec bit and byte (a link by its target), sorted, capped at 4096 entries or 64 MiB; the run's is every command word plus that | `core/artifact/regenerate.rs:169-186`, `:275-346` | `core/artifact/regenerate/tests.rs:250`, `:266`, `:280`, `:308` |
| Approval | Granted by "Approve and run", which sends the fingerprint it showed; the app resolves again and refuses if the code moved since. Checked again right before each run | `ui/ArtifactRegenerate.tsx:78-100`, `:143-158`; `tauri/regenerate.rs:166-206`, `:241-252` | `tauri/regenerate.rs:471`, `:506`; `ui/ArtifactRegenerate.test.tsx:74` |
| Approval | Store: `<config>/artifact-regenerate-approvals.json`, **Machine, device-bound**, keyed by the page approval's key. Its entries are not filtered for malformed ones as the page record's are; one with a fingerprint but no `pin` reads as `New` | `core/artifact/regenerate.rs:49`, `:433-439`, `:466`, `:474-500`; `pf:8722` | — |
| Approval | **Invalidated by** a code edit or a command word (`Changed`), a pin's folder, command or cwd (`PinChanged`), an old short id. **Not** by unpinning (there is no `forget`), so a re-pin of the same folder, command and cwd stays approved while the page asks again. A workspace **rename loses** it (only the page approval is `moved`) | `core/artifact/regenerate.rs:444-468`; `core/artifact/mod.rs:436`; `core/wscmd/rename.rs:1298` | `core/artifact/regenerate/tests.rs:250`, `:295`, `:308`, `:587`; the unpin and rename cases: UNTESTED |
| Approval | **On re-ask** a dialog, "Run the code that regenerates `<name>`?", naming the folder, with "The code changed since you approved it", "This pin's command changed…" or "You have not approved it on this machine". A refused run is a `Notice` (cause `artifact-regenerate-failed:<folder>`, fix "Regenerate again") | `ui/ArtifactRegenerate.tsx:118-176`; `tauri/regenerate.rs:277-290` | `ui/ArtifactRegenerate.test.tsx:74`, `:91` |
| Approval | Independent of the page's script approval: it runs for a page whose template was never approved, writing what that template declares | `tauri/regenerate.rs:93-101` | — |
| Recovery | The sandbox: a session-host Shell program, Seatbelt on macOS, `checked_wrap` on Linux; no network; writes only to scratch; reads in the plane only the code and artifact folders. Windows: refused (`NOT_HERE`) | `core/artifact/regenerate.rs:74-78`, `:657-783`; `tauri/regenerate.rs:218-222`, `:300-359` | `core/artifact/regenerate/tests.rs:504`, `:541`, `:634`, `:667`, `:706`; `ui/ArtifactRegenerate.test.tsx:123` |
| Recovery | **Fingerprint changed between hash and run:** the hash is taken at `resolve`, and the run then reads the whole code folder itself, so bytes changed in between are not seen | `tauri/regenerate.rs:13`, `:242`; `core/artifact/regenerate.rs:763-766` | UNTESTED |
| Recovery | The sandbox cannot be built: the error is returned straight up, **without** the "last good" date that errors after the run carry; both reach the same Notice | `core/artifact/regenerate.rs:669-679`, `:697-717`; `tauri/regenerate.rs:240-259`, `:393-396` | UNTESTED |
| Recovery | Non-zero exit, 300 s timeout, bad output (not JSON, too large, a link, none): nothing is written, last good data stands, the sentence names it | `tauri/regenerate.rs:351-357`, `:379-398`; `core/artifact/regenerate.rs:514-606` | `tauri/regenerate.rs:434`; `core/artifact/regenerate/tests.rs:388`, `:414`, `:441`, `:454`, `:475`, `:488` |
| Recovery | No session host, no link in 10 s, host ending mid-run: each a sentence. "Regenerate again" in the Notice retries with **no restart**; a changed fingerprint opens the dialog again. Whether the app brings a dead session host back was not traced | `tauri/regenerate.rs:40`, `:234-236`, `:259`, `:353`; `ui/ArtifactRegenerate.tsx:45-57`, `:102`, `:123` | `tauri/regenerate.rs:459`; `ui/ArtifactRegenerate.test.tsx:102` |

## 5. Live subscriptions

The core hub is `core/artifact/live.rs`; it holds no clock, it says Run, Hand or WakeAt and its
caller acts. The app driver is `tauri/artifacts/live.rs`. A page sends `{id, subscribe:
"<id>@<N>", params}` on its port (`tauri/artifacts.rs:105-140`).

| Facet | Behaviour | Evidence | Test |
| --- | --- | --- | --- |
| Versioning | A feed is keyed by `<id>@<N>` as declared, parameters, plane and workspace. A version mismatch is refused at admission like a call: one answer, nothing subscribed. The answer envelope carries no version | `tauri/artifacts/live.rs:45-80`, `:166-202`; `core/catalogue.rs:532-559` | `tauri/artifacts/live.rs:443`; `core/catalogue/tests.rs:41`, `:56`; the subscribe-time refusal: UNTESTED |
| Versioning | An id no longer catalogued when its feed runs answers `failed` | `tauri/artifacts/live.rs:76-79`, `:316-321` | UNTESTED |
| Approval | A subscribe is admitted exactly as a call is (`calls::admitted`). Subscriptions are in memory only | `tauri/artifacts/live.rs:90-95`, `:181-191`; `tauri/artifacts/calls.rs:227-250` | `tauri/artifacts/calls.rs:364`; `tauri/artifacts.rs:2091` |
| Approval | Every push is gated again for every page: a changed load, a gone webview or a lost approval unsubscribes it, and a lost approval sends `not-approved` once | `tauri/artifacts/live.rs:336-366` | UNTESTED |
| Recovery | A plane query re-runs on a plane-watch change of the kinds it reads; `purlis.chats` on the app's chat moves. One run in flight per feed, at most one start a second, a change during a run asks once more | `core/artifact/live.rs:37`, `:41-73`, `:217-240`; `tauri/artifacts/live.rs:214-283` | `core/artifact/live.rs:275`, `:316`, `:341`, `:411`, `:453`, `:469`; `tauri/artifacts/live.rs:499` |
| Recovery | Freshness: only `forge_milestones` (`fresh_seconds = 60`) and extension queries re-run on a clock. A feed with a freshness re-hands every answer; one without hands only a changed one. The page gets only `at`; purlis draws nothing about a live answer's age. The window's 24 h "stale" (`STALE_AFTER`) is a separate mechanism, for data files | `core/catalogue.rs:81`; `core/catalogue/milestones.rs:33`; `core/artifact/live.rs:181-193`, `:224-229`; `core/artifact/fresh.rs:1-27` | `core/artifact/live.rs:480`, `:508`, `:518`, `:543`; `core/catalogue/tests.rs:211` |
| Recovery | Forge offline, auth gone, or rate limit: `failed` with a sentence, re-asked every 60 s with **no backoff**; recovers with **no restart** once the forge answers. The hub ignores the limit's `reset` though the sentence says "purlis waits"; only purlis's own budget holds a background request back | `core/catalogue/milestones.rs:178-189`; `core/forge/budget.rs:27-33`, `:331` | `core/catalogue/milestones/tests.rs:203`; `core/forge/budget.rs:588`; recovery after sign-in: UNTESTED |
| Recovery | Stale data: the hub keeps only the last answer, and an error replaces it, so a later subscriber gets the error, not the last good data | `core/artifact/live.rs:139-143`, `:194` | `core/artifact/live.rs:559` |
| Recovery | Subscriber gone: unsubscribe, reload or webview gone drops it, and a feed with no subscriber is dropped | `core/artifact/live.rs:148-155`; `tauri/artifacts/live.rs:155-160`, `:349-356` | `core/artifact/live.rs:366`, `:384`; `tauri/artifacts/live.rs:410`, `:462` |
| Recovery | **A run that never finishes wedges its feed:** only `finished` clears `running`, and the hub has no timeout | `core/artifact/live.rs:221-237`; `tauri/artifacts/live.rs:296-306` | UNTESTED |
| Tests | No end-to-end test of a subscription: `artifact-queries.e2e.ts` covers declared calls only | `app/e2e/specs/artifact-queries.e2e.ts:113-147` | UNTESTED |

## 6. Hook lines

A harness's hook writes a line to `app/hooks.sock`. When a session host holds the project, it
binds the socket, issues the tokens and admits the lines (`crates/host/src/hosting.rs:16-20`).
ADR 0076: hooks are one of the named causes that move a run.

| Facet | Behaviour | Evidence | Test |
| --- | --- | --- | --- |
| Versioning | A plain line has no version field; its kind is serde's untagged shape, each newer kind requiring a field older ones lack | `hw:2655-2685`, `hw:1184-1214` | `core/hookwire/tests.rs:4773` |
| Versioning | A line the app cannot read gets `UNKNOWN_LINE` ("from a purlis this app does not know") and the connection closes; the channel runs on. **Garbage and a newer line get the same answer** | `hw:1197-1206`, `hw:4443-4446`, `hw:4514-4523` | `core/hookwire/framed/tests.rs:252`; `core/hookwire/tests.rs:1768` |
| Versioning | A line over `A_LINE_IS_AT_MOST` is refused with a reason | `hw:4508-4513` | the bound: `core/hookwire/tests.rs:430`; the refusal sent: UNTESTED |
| Versioning | The framed stream has `framed::VERSION = 1`; a hello at another version gets `Answer::No` naming both. A client that hears nothing or `UNKNOWN_LINE` falls back to plain lines | `core/hookwire/framed.rs:45-46`, `:186-200`; `hw:4791-4803` | `core/hookwire/framed/tests.rs:293`, `:306`, `:333` |
| Versioning | Each spool line carries `v: 1`; an unparsable one is `Why::Unreadable`. Spool `keys.json` reads versions 1 and 2 and refuses others ("a newer purlis may have written it") | `core/hookwire/spool.rs:95-104`, `:855`, `:968-1004`, `:1809-1816` | UNTESTED for any other `v` or keys version |
| Versioning | The harness's own payload version is never checked; an unrecognisable conversation is `Foreign` or `Unknown`, never evidence | `hw:166-177` | `core/hookwire/tests.rs:1418`, `:1434` |
| Approval | Socket `0600` in a private folder, symlinks refused, a stale socket removed before bind. A peer of another uid is closed unread (kernel peer credentials) | `hw:3879-3911`, `hw:4109-4120`; `crates/same-user/src/lib.rs:135-144` | `core/hookwire/tests.rs:1977`, `:2257`, `:4204`; `crates/same-user/src/lib.rs:531` |
| Approval | A per-chat 32-byte token, in memory only, compared in constant time; a line is admitted only from the chat's recorded program or a process inside it (D-1407-6) | `hw:564-580`, `hw:609-625`, `hw:761-796`, `hw:997-1023`, `hw:1119` | `core/hookwire/tests.rs:4099`, `:4273`, `:4707`; `crates/same-user/src/ancestry/tests.rs:114` |
| Approval | Tiers: the socket and the spool keys (an HMAC of the token, `app/spool/keys.json`) are **Clone state, transient**. Invalidated by the program's end, `forget` or a new token | `core/hookwire/spool.rs:33-38`; `pf:7699`, `pf:7922`; `hw:931-968` | `core/hookwire/spool/tests.rs:206`, `:219`; `core/hookwire/tests.rs:4565` |
| Approval | **What the sender is told:** no token, "this line does not carry chat N's token"; outside, `OUTSIDE_THE_CHAT`; unbound, `UNCONFIRMED_PROGRAM` ("restart the chat"). A comment says a tokenless line is dropped without a word; the code answers it | `hw:571`, `hw:640-671`, `hw:4535-4560` | `core/hookwire/tests.rs:4128`, `:4313`, `:4632` |
| Recovery | A hook never breaks a turn: one attempt, then the spool, `Ok(Spooled)`, on no listener, no receipt in time, or `Answer::No`. A refused receipt is `PermissionDenied` and spools nothing | `hw:11-13`, `hw:2502-2507`, `hw:3582-3640`, `hw:4906-4913` | `core/hookwire/tests.rs:1396`, `:4632`; `core/hookwire/delivery_tests.rs:89`, `:109`, `:131` |
| Recovery | With no token, or outside the sandbox-allowed folder, the line is lost and the hook says so on stderr | `hw:35-37`, `hw:3626-3631`, `hw:3924-3935` | `core/hookwire/delivery_tests.rs:148`, `:169` |
| Recovery | The spool drains with **no restart**: at host start, chat end, and within seconds for a running chat; bad MAC, missing key, a gap or a repeat is reported, never dropped silently | `core/hookwire/spool.rs:9-14`, `:1249-1278` | `core/hookwire/spool/tests.rs:135`, `:164`, `:189`, `:2244`, `:2290` |
| Recovery | The window restarts while a chat runs: the host keeps the program, channel and tokens for a 10-minute grace | `crates/host/src/lifetime.rs:6-17` | `crates/host/tests/it/a_window_gone_past_the_grace_period_ends_its_chats_as_a_quit.rs:258`, `:273` |
| Recovery | **A newer line meeting an older host** is answered `UNKNOWN_LINE`, spooled under `v: 1`, and unreadable when that host drains it: never recovered, nothing says to upgrade | `hw:4514-4523`; `core/hookwire/spool.rs:1809-1816` | UNTESTED |
| Recovery | **The session host restarting while a chat's process lives:** tokens are in memory only, so every line spools as tokenless. Whether a chat may outlive its host is not stated | `hw:567` | UNTESTED |
| Recovery | Floods and hangs are bounded per peer and in total | `hw:4121-4135` | `core/hookwire/tests.rs:4838` |

## 7. Session protocol

`crates/session-protocol`: the window, the CLI or another host to the session host over
`purlisd.sock`. It is not a harness surface: a client inside a chat is refused every scope
(`crates/session-protocol/src/version.rs:115-118`, `crates/session-protocol/src/local.rs:91`).

| Facet | Behaviour | Evidence | Test |
| --- | --- | --- | --- |
| Versioning | `MAGIC`, then a JSON hello naming one version per major; the host answers the highest shared major at the lower minor, speaks its own major and the one before, and fails closed. Unknown fields within a major are ignored | `crates/session-protocol/src/version.rs:4-24`, `:86-127` | `crates/session-protocol/tests/it/the_version_is_negotiated_and_an_unknown_major_is_refused.rs:28`, `:35`, `:43`, `:59`, `:85` |
| Versioning | A handshake over 10 s is `TimedOut`; a hello over 4096 bytes is `TooLong` | `crates/session-protocol/src/version.rs:48`, `:86-127` | UNTESTED |
| Approval | A scope is admitted with a credential; the host must prove it minted it (`HostUnproven`); a peer inside a chat is refused whatever its proof. Credentials live in the machine store | `crates/session-protocol/src/version.rs:86-120`; `crates/session-protocol/src/local.rs:88-100`; `core/hostsocket.rs:19-20` | `crates/session-protocol/tests/it/a_client_started_inside_a_chat_is_refused_every_human_scope.rs`; `crates/session-protocol/src/local/tests.rs:199` |
| Recovery | The window gone: the grace period applies; a view re-attaches by epoch, with backoff | `crates/host/src/lifetime.rs:6-10` | `crates/session-protocol/tests/it/a_view_is_attached_again_by_epoch_with_backoff.rs:66` |

## 8. ACP (a chat at level 3)

ADR 0073: the structured protocol of a level-3 chat is ACP over the agent's stdio
(`core/acp.rs:4-5`).

| Facet | Behaviour | Evidence | Test |
| --- | --- | --- | --- |
| Versioning | `initialize` offers V1; any other answer is `NotStarted::Version` naming both, or a `Handshake` error. The agent's name and version are recorded | `core/acp.rs:283-293`, `:1487`, `:1496-1513` | `crates/fake-harness/tests/over_acp.rs:148`, `:163` |
| Versioning | A stop reason from a later protocol is `Unknown`, never read as a cancel | `core/acp.rs:262-264` | `core/acp/tests.rs:237` (partly) |
| Approval | No `fs` and no `terminal` offered; a client method not offered is refused and reported; `authenticate` is never called (a login need is `NotStarted::Login`); a permission ask is answered only from a human scope, with no deadline | `core/acp.rs:14-17`, `:30-36`, `:1515-1524` | `over_acp.rs:101`, `:182`, `:336`, `:407` |
| Recovery | A line over 16 MiB ends the chat; the agent's process group ends with the connection; a turn whose agent exited fails `TurnFailed::Gone` | `core/acp.rs:44-50`, `:268-271`, `:323` | `core/acp/tests.rs:221`, `:387`; `over_acp.rs:423` |
| Recovery | The agent crashing mid-turn gives `Gone` and `Event::Ended` | `core/acp.rs:131-132`, `:270` | UNTESTED |
| Recovery | Handover on upgrade, the guard's first look and "allow always" are deferred (HP-16, FD-28) | `core/acp.rs:59-60` | — |

## 9. Harness declarations and the plugin

| Facet | Behaviour | Evidence | Test |
| --- | --- | --- | --- |
| Versioning | A declaration has no schema version; `tested` (for example `"2.1.283"`) is free text shown on the harness card. **The installed harness's version is never detected**: no `--version` probe exists | `core/harness_declaration.rs:303-304`, `:998-1002`; `core/harness_declaration/claude.toml:12`; `core/harness_card.rs:241-244` | `core/harness_declaration/tests.rs:48` |
| Versioning | Declarations are read with `deny_unknown_fields`, so a field from a newer purlis makes an older one refuse the declaration | `core/harness_declaration.rs:869-870` | UNTESTED |
| Versioning | The bundled plugin has no `version`; an app chat loads the bundle's copy with `--plugin-dir` at each launch. An installed copy is stale when it differs from what installing now would write | `app/src-tauri/plugin/.claude-plugin/plugin.json`; `core/plugin.rs:13-23`; `core/plugin_install.rs:461-500` | `core/plugin_install/tests.rs:546`, `:671` |
| Approval | A project declaration's program runs only after the operator approves its whole digest: missing is `New`, different is `Changed`. A declared `hooks = true` is refused (level 1) | `core/harness_declaration.rs:16-17`, `:29-32`; `core/profiletrust.rs:303-327` | `crates/purlis-core/tests/it/a_terminal_only_harness_runs_from_a_declaration_alone.rs:234`, `:410`, `:440` |
| Approval | Store: `.charter/harness-declarations-approved.json`, **Clone state** (every other code approval in this page is Machine, device-bound) | `core/harness_declaration.rs:53-54`; `pf:1113-1119` | — |
| Approval | Plugin pins: `purlis@inline` always on; `charter@charter` and old ids always off; old `[harness_plugins]` keys ignored and an Inbox ask proposes removing them | `core/harness_plugin.rs:11-19`; `core/harness_plugin/old.rs:3-12` | `core/harness_plugin/old/tests.rs` |
| Recovery | After an app upgrade, launch refreshes an installed copy unasked, only in a release build that is not fenced; never installs where none was, never repoints a copy another charter on disk runs | `core/plugin_install.rs:461-500`, `:678-680`; `tauri/lib.rs:414`, `:426` | `core/plugin_install/tests.rs:690`, `:700`, `:722`, `:867`, `:880`; the scenario suite never runs a refresh (debug builds skip it) |

## 10. CLI machine output and exit codes

Readers are chats (skills tell them to run `--json`, for example
`app/src-tauri/plugin/skills/purlis-theme/SKILL.md:31`), the plugin build (`hook --list --json`,
`core/plugin.rs:222`) and CI. The app reads no CLI JSON; it calls the core in-process
(`ui/bindings.ts:2521`).

| Facet | Behaviour | Evidence | Test |
| --- | --- | --- | --- |
| Versioning | **No common version.** `queries --json` carries `api: 1`; `hook --list --json` and `theme list/check --json` carry `schema: 1`; `doctor --json`, `skill history --json`, pool and poolsetup carry nothing. Nothing reads these keys, and nothing defines what a newer `schema` means | `cli/main.rs:514-518`, `:774-776`; `core/hookreg.rs:235`; `cli/theme.rs:66-72`; `core/doctor/mod.rs:650-668`; `cli/skill.rs:1905-1906` | `crates/purlis-cli/tests/it/purlis_queries_lists_the_catalogue.rs:30`; `core/hookreg.rs:263`; `crates/purlis-cli/tests/it/theme.rs:763`; `core/doctor/tests.rs:67`. Versioning of the rest: UNTESTED |
| Versioning | A CLI and an app of different versions: an answer the CLI has no arm for is an internal failure (70) that says "this command and the app may be different versions" | `cli/dispatch.rs:1007-1014`; `cli/internal.rs:35-41` | `crates/purlis-cli/tests/it/dispatch_from_inside_the_app.rs:1842` |
| Versioning | The recorded fixture freezes 601 scenarios (CLAUDE.md says 404); 29 use `--json`; exit codes 0 ×423, 1 ×160, 2 ×16, 7 ×1, 127 ×1 | `tests/fixtures/recorded/behaviour.jsonl`; `crates/purlis-cli/tests/recorded_behaviour/main.rs:11-27` | the replay itself |
| Approval | None of its own: a command run by a chat is part of that chat and its sandbox | `core/forge/route.rs:10-13` | — |
| Recovery | Exit 70 only for an internal failure (a panic, an unhandled I/O error), with one stderr line; hook, mcp, serve, shell-guard, bridge and listen keep their own end. A clap usage error exits 1 | `core/caught.rs:22`, `:85-89`; `cli/clierror.rs:40-45`; `cli/internal.rs:18-41`; `cli/main.rs:5433-5436` | `crates/purlis-cli/tests/it/an_internal_failure_exits_with_its_own_code.rs:32`, `:39`, `:98`, `:123`, `:152`; `cli/clierror.rs:107` |
| Recovery | **Exit 2 is still a refusal code** in `doctor`, `theme`, `scan` and `listen`, against ADR 0120's "a refusal … exits 1"; 16 recorded scenarios freeze it (for example `secret-cp-refuses-a-symlink`) | `docs/adr/0120-a-purlis-command-exits-70-for-an-internal-failure-and-only-then.md:17-19`; `cli/main.rs:5969`; `cli/theme.rs:514`, `:606`, `:830`, `:845`; `cli/scan.rs:24`, `:30`; `cli/listen.rs:119`, `:125` | the recorded scenarios |
| Recovery | Unknown first word: tried as an extension command, else exit 1. With `--fix --json`, fix lines go to stderr | `cli/main.rs:5428-5436`, `:5984-5985` | stdout staying clean: UNTESTED |

## 11. The forge

| Facet | Behaviour | Evidence | Test |
| --- | --- | --- | --- |
| Versioning | The native transport pins `x-github-api-version: 2022-11-28`; the CLI transport sends the argv the Python charter sent. No minimum `gh` or `glab` version is declared or checked | `core/forge/http.rs:194`; `core/forge/cli.rs:7-8` | `core/forge/http.rs:803`; `crates/purlis-core/tests/it/a_forge_cli_is_asked_exactly_what_python_asked.rs` |
| Versioning | `forge-accounts.json` `version` is **read and ignored**; a write always stamps `1`, keeping unknown keys, so a newer build's file is rewritten as version 1 | `core/forge/accounts.rs:61-62`, `:606-616`, `:689` | UNTESTED |
| Approval | Only the person in the window, on a signed-in account, gets the native transport and its token. Chats, MCP calls and `purlis` commands get `gh`/`glab` with its own login. Only the host signs in or out | `core/forge/route.rs:10-15`; `core/forge/signin.rs:7-13`; `core/forge/accounts.rs:31-33` | `crates/purlis-core/tests/it/a_forge_token_never_reaches_a_chat.rs`; `a_signed_in_forge_token_never_reaches_a_chat.rs`; `a_chats_process_tree_is_handed_no_forge_sign_in.rs` |
| Approval | Token in the **Keyring** (`purlis/@forge/<host>/<id>`); account list `<config>/forge-accounts.json`, **Machine, device-bound**. Invalidated by sign-out, or by the forge refusing a refresh, which marks it signed out | `core/forge/accounts.rs:7-15`; `pf:8746`, `pf:8748`; `core/forge/revoke.rs:4-17`; `core/forge/refresh.rs:199-213` | `crates/purlis-core/tests/it/forge_accounts.rs:126`; `forge_device_sign_in.rs:551` |
| Approval | **On re-ask:** "purlis's sign-in to `<host>` as `<login>` has ended … Sign in again in Settings", or "… has no token in the keychain any more" | `core/forge/refresh.rs:190-196`, `:216-222` | `forge_device_sign_in.rs:551` |
| Recovery | Failure kinds: Auth, Forbidden, NotFound, RateLimited{reset}, Conflict, Unavailable, Transport, Unrecognised, from the HTTP status. **On the CLI route there is no status, so an expired `gh` login is Unrecognised, never Auth**, for every chat and command | `core/forge.rs:706-726`; `core/forge/transport.rs:210-225` | `crates/purlis-core/tests/it/the_native_transport_asks_conditionally_and_once.rs:383` |
| Recovery | Offline, a missing CLI or a deadline (10 s auth, 60 s listing) is Transport; a failure is never retried on the other route | `core/forge/transport.rs:256-279`, `:295-296`; `core/forge.rs:77-80` | `crates/purlis-core/tests/it/a_workspace_s_finished_folders_are_pruned.rs:551`, `:888` |
| Recovery | purlis's own allowance is 1,000 counted requests an hour per account; background requests are held back with a reset, a person waiting never is; kept in `<config>/forge-budget/` and survives a restart. Its own comment says nothing in the app drives it yet | `core/forge/budget.rs:7-8`, `:28-35`, `:57`, `:314-339`; `pf:8727` | `core/forge/budget.rs:588`, `:655` |
| Recovery | Sign-in refreshed ahead of expiry; after the forge ends one, signing in again in Settings recovers with **no restart**. Panels read forge state from a cache and never fetch | `core/forge/refresh.rs:4-16` | `forge_device_sign_in.rs:507`; `crates/purlis-core/tests/it/forge_state_is_read_from_a_cache_and_never_fetched.rs:58` |

## 12. Dispatch

CLI or MCP to the app over the hook socket (section 6 for its handshake), plus its own stores.

| Facet | Behaviour | Evidence | Test |
| --- | --- | --- | --- |
| Versioning | Queued messages (`spool/messages/<chat>/`) carry `v: 1`; one of **another `v`**, or a bad MAC, is **removed and never delivered**, with only a log warning | `core/dispatchqueue.rs:48-49`, `:213-215`, `:624-628`; `core/hookwire/relay/carrying.rs:216-224` | the MAC case: `core/dispatchqueue/tests.rs:76`; another `v`: UNTESTED |
| Versioning | A dispatch record of another version is skipped and **left as it is**; an away record of another version lists nothing, is left, and refuses a write; a chain begun under an older purlis is refused | `core/dispatchrecord.rs:13-14`, `:72`; `core/dispatchaway.rs:75`, `:85-86`, `:302-306`; `core/dispatchlimits.rs:787` | `core/dispatchrecord/tests.rs:539`; `core/dispatchaway_tests.rs:516`; `core/dispatchdecision.rs:3326`, `:3506` |
| Approval | A grant is per pair at three levels: this chat (memory), this machine (`app/sandbox.json`), the project (committed `[dispatch.grants]`). A pulled project grant waits for this machine's yes, bound to what it accepted. A "never" is `app/dispatch-never.json` and fails closed; a policy file can lock pairs | `core/dispatchgrant.rs:11-52`; `core/dispatchnever.rs:4-25`; `pf:866` | `core/dispatchgrant_tests.rs:33`, `:46`, `:59`, `:132`, `:598`, `:655`; `core/dispatchnever_tests.rs:206`, `:391` |
| Approval | Nothing a chat sends makes or widens a grant, but that holds only by the sandbox: with it off, a chat can write both files (documented) | `core/dispatchgrant.rs:34-42` | — |
| Approval | **On re-ask** a Notice with the first brief, whole, escaped, and marked when cut | `core/dispatchgrant.rs:4-7` | `core/dispatchgrant_tests.rs:399`, `:412`, `:440` |
| Recovery | While the app is down, tell, note, ask and answer queue durably and arrive once, in order, on the next contact; an undeliverable one shows in `dispatch list`. A wait that runs out says the task still runs | `core/dispatchqueue.rs:4-16`; `core/dispatched.rs:23-27` | `core/dispatchqueue/tests.rs`; `dispatch_from_inside_the_app.rs:1724`, `:1767` |

## 13. Other surfaces

| Surface | Version carrier | Older or newer | Evidence | Test |
| --- | --- | --- | --- | --- |
| Project file (`charter.toml`) | `schema` (absent = 1; this build reads 2) plus `requires = [{feature, since}]` | an unknown schema or a missing feature makes the project read-only and names `since`; old forms read for 6 months | `pf:447-500`; `core/compat.rs:12`, `:15`, `:203` | `core/compat.rs:352`, `:384`, `:401` |
| Sub-format files (reopen manifest, records) | a per-file `version` or `v` | exact match, else ignored whole (varies) | `pf:6261`, `pf:6851` | per module |
| Themes | format 1 or 2 by shape; `$schema` URL checked only to be a string | an unknown token is said and left out; an unknown key refuses the theme and draws the default | `core/theme.rs:14-30`; `core/theme/format.rs:385-391`; `docs/schema/theme.v2.schema.json` | `core/theme/format_tests.rs:503`; `tools/theme-schema-since-release.test.mjs` |
| Skill grades | `VERSION = 1` | a newer row is skipped | `core/skills/grades.rs:35`, `:829`, `:1047` | UNTESTED |
| `purlis://` links | none: four places | anything else refused | `core/artifact/places.rs:4-27` | section 2 |
| App updater | signed manifest whose trusted comment names the version | guarded at release time | `app/src-tauri/tauri.conf.json:60-63`; `crates/release-manifest/src/lib.rs:4-33` | `crates/release-manifest` |
| Browser engine | pinned build; a newer manifest only if its Sigstore bundle verifies and it moves forward | fails closed | `crates/browser-engine/src/install.rs:4-27` | `crates/browser-engine` |
| Browser engine wire (CDP over pipes) | none | — | `crates/browser-engine/src/cdp.rs:4-11` | UNTESTED |

## Gaps

Each gap is tagged:

- **UNTESTED:** a defined behaviour that no test proves.
- **INCONSISTENT:** two surfaces solve the same problem differently.
- **UNDEFINED:** nothing in the code defines what happens.

### Versioning

1. **INCONSISTENT:** a record or line of an unknown version gets one of five answers, depending
   on the store:
   - refused whole: the extension manifest, spool `keys.json`, the dispatch away record and a
     page declaration;
   - skipped and left: the dispatch record and skill grades;
   - **deleted silently**: a queued dispatch message;
   - **downgraded on write**: `forge-accounts.json`;
   - written and **never read**: the `version` of `artifact-approvals.json` and of the
     Regenerate record.

   (Sections 1, 2, 4, 6, 11, 12, 13.)
2. **INCONSISTENT:** a garbage version is reported as a missing one ("says no version"), and a
   garbage hook line as "from a purlis this app does not know" (sections 1, 6).
3. **UNDEFINED:** a page's `@N` selects no behaviour, and no version can be retired. An
   extension query's `@N` is bounded by purlis's `API`, not the extension's (section 2).
4. **INCONSISTENT:** `window.purlis.api` is a literal `1`, not `catalogue::API`. `since` is
   `0.5.1` for five queries that first shipped in 0.5.2, and is spelled two ways (sections 2, 3).
5. **INCONSISTENT:** the CLI's `--json` outputs carry `api`, `schema` or nothing, and no reader
   checks any of them (section 10).
6. **UNDEFINED:** which MCP protocol versions purlis supports is whatever `rmcp` 3.5 supports
   (section 3).
7. **UNDEFINED:** a newer hook line meeting an older host is spooled under `v: 1`, then
   unreadable, and never recovered (section 6).
8. **UNDEFINED:** the installed harness's version is never detected. A declaration has no schema
   version and is parsed strictly, so a newer purlis's declaration is refused by an older one
   (section 9).
9. **UNDEFINED:** no minimum `gh` or `glab` version (section 11).
10. **UNTESTED:**
    - Extension protocols 6 to 8 have no frozen fixture, though `extension-api.md` says each
      protocol is frozen.
    - A garbage manifest `version`.
    - `PURLIS_PROTOCOL`.
    - A version-8 extension answering a query.
    - Spool `v` other than 1, and `keys.json` versions.
    - The session protocol's handshake timeout and size limit.
    - `forge-accounts.json` at another version.
    - A dispatch message at another `v`.
    - `deny_unknown_fields` on declarations.
    - `since` against any release.

### Approval

11. **INCONSISTENT:** the page approval and the Regenerate approval follow different lifecycles
    for the same pin. Unpinning forgets the first and keeps the second; a workspace rename
    carries the first and loses the second. The Regenerate record is also validated less
    strictly (section 4).
12. **INCONSISTENT:** code approvals sit in different tiers. A harness declaration's approval is
    Clone state; an extension's, a page's and Regenerate's are Machine, device-bound
    (sections 1, 2, 4, 9).
13. **INCONSISTENT:** "may a chat call this without asking" depends on the harness. `PRE_ALLOWED`
    reaches Claude Code only, and on Codex and opencode the same 18 reads prompt (section 3).
14. **INCONSISTENT:** the re-ask is drawn four ways:
    - the extension dialog;
    - a sentence in the page bar's `<span>`, not a `Notice`;
    - the Regenerate dialog and its `Notice`;
    - the harness's own prompt, which purlis does not define.

    Re-installing a changed extension shows the first-ask dialog, not the re-ask one, and
    comments say "the next launch asks" where nothing does (sections 1 to 4).
15. **INCONSISTENT:** one query answers as two callers. A page asks the forge as the person; a
    chat asks as itself through `gh` (sections 3, 11).
16. **UNDEFINED:** Regenerate runs for a page whose template was never approved, writing what
    that unapproved template declares (section 4).
17. **UNDEFINED:** an approval binds bytes, not the scope of the queries it declares, so a later
    purlis or extension change that widens a query is covered by an old approval (section 2).
18. **UNDEFINED:** dispatch grants are protected only by the sandbox (documented as unprotected
    without it) (section 12).
19. **UNTESTED:**
    - The "Trust it anyway" copy.
    - The Regenerate unpin and rename cases.
    - The push-time re-gate of a live feed.

### Recovery

20. **INCONSISTENT:** one cause gets different failure vocabularies:
    - **an extension query:** `needs-extension`, `not-approved` or `failed`, for one failure;
    - **a page:** `not-approved` for a refused declaration, with the reason never shown;
    - **MCP:** `invalid_params` or `isError`;
    - **the forge on the CLI route:** Unrecognised in place of Auth;
    - **a timeout:** any query error after 5 s is called one.

    (Sections 1 to 3, 11.)
21. **UNDEFINED:** a page's ask can get **no answer at all** (malformed, over 8 KiB, before
    `open`, to a closed load), and the port script has no timeout (section 2).
22. **UNDEFINED:** a live feed whose run never finishes is wedged, since the hub has no timeout
    (section 5).
23. **INCONSISTENT:** retry promises differ from retry behaviour:
    - "It asks again in a minute" holds for a subscription, not for a one-shot query or an MCP
      chat.
    - "purlis waits" on a rate limit, while the hub re-asks every 60 s, ignoring `reset`.
    - An extension gets one try with no backoff.
    - purlis's own MCP server gets no restart at all.

    (Sections 1, 3, 5.)
24. **INCONSISTENT:** "fresh" means two things: a live feed's `fresh_seconds` and a data file's
    24 h `STALE_AFTER`. A live error also replaces the last good answer for later subscribers
    (section 5).
25. **UNDEFINED:** purlis's own MCP server crashing, and a tool added mid-chat (no
    `tools/list_changed`) (section 3).
26. **UNDEFINED:** the session host restarting while a chat's process lives: its tokens are gone,
    so every line spools (section 6).
27. **UNDEFINED:** Regenerate fingerprints the code, then runs code it reads later; and errors
    before a run carry no "last good" date (section 4).
28. **UNDEFINED:** whether purlis's forge budget is enforced today (its own comment says nothing
    drives it) (section 11).
29. **INCONSISTENT:** a refusal exits 1 or 2 depending on the command, against ADR 0120
    (section 10).
30. **UNTESTED:**
    - The silent refused declaration in the window.
    - A dropped ask.
    - A plane no longer held.
    - A live subscription end to end.
    - Recovery after sign-in.
    - `forge_milestones` over MCP.
    - A Regenerate sandbox that cannot be built.
    - An ACP agent crashing mid-turn.
    - A plugin refresh in any scenario.
    - The deprecated and replaced id warnings.
    - The hook line-too-long refusal.
    - `--fix --json` keeping stdout clean.

### Documentation drift found on the way

31. **INCONSISTENT:**
    - `hw:571` says a tokenless line is dropped without a word; the code answers it.
    - `pf:8721` says the approval key is a SHA-256; the code keeps 16 bytes of one.
    - `core/artifact/live.rs:94-97` says no core query has a freshness; `forge_milestones` has
      one.
    - `CLAUDE.md` says the recorded fixture has 404 scenarios; it has 601.

## Seams

Five places where one shared seam would remove the most inconsistency. They are named here, not
designed.

1. **A versioned reader for every record and line.** This covers gaps 1, 2, 5, 7 and 10. Every
   store and wire above reads its own `version` or `v`, or none, and decides on its own whether
   to refuse, skip, delete, downgrade or ignore. One reader would answer older, newer, unknown
   and garbage the same way everywhere, and would say what the person or caller is told.
2. **One approval ledger for code that runs.** This covers gaps 11, 12, 14, 16 and 17. Six
   stores approve code or reach:
   - `extensions.json`;
   - `artifact-approvals.json`;
   - `artifact-regenerate-approvals.json`;
   - `harness-declarations-approved.json`;
   - `artifact-link-hosts.json`;
   - the dispatch grants.

   Each has its own key, tier, validation, lifecycle on unpin, rename and move, and re-ask
   drawing. The lifecycle events and the re-ask are where they disagree most.
3. **One refusal and failure vocabulary across the boundary.** This covers gaps 2, 20, 21, 25
   and 29. Each surface has its own set:
   - `needs-extension`, `not-approved`, `failed` and `timeout` for a page;
   - `invalid_params` and `isError` for MCP;
   - Auth and Unrecognised for the forge;
   - `UNKNOWN_LINE` for a hook;
   - exits 1, 2 and 70 for the CLI;
   - silence for a dropped ask or a refused declaration.

   A shared set, carrying a reason the window can show, would close the silent cases.
4. **One retry and freshness policy.** This covers gaps 22, 23, 24, 26 and 28. The live hub's
   fixed 60 s, the forge's `reset` and budget, the extension's single 5 s try, the MCP server's
   no-restart, the hub's missing timeout and the two meanings of "fresh" each decide alone when
   to ask again. The copy promises behaviour only some of them have.
5. **One contract register for what shipped when.** This covers gaps 3, 4, 6, 8, 9 and 10. The
   following each record "which release answers what" separately, or not at all:
   - query `since`;
   - per-query `@N`;
   - `catalogue::API` and the port script's literal `1`;
   - each extension capability's `since()`;
   - the frozen extension fixtures;
   - the MCP protocol;
   - a harness's `tested` version.

   Tying them to one register, checked against the release tags, is what would make protocol
   v9 or `api 2` safe to ship.
