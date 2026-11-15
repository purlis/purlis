# The record store and the approval ledger: call sites, contract tests and tickets

This note is the working half of
[ADR 0125](../adr/0125-one-private-record-store-with-a-versioned-read-and-one-approval-ledger-for-code-that-runs.md),
for epic #6677. The ADR decides. This note lists what moves: every private record that would
move onto the record store, every approval adapter with its record, key and lifecycle callers,
the contract tests that must exist before anything moves, and a proposed order of tickets. It
proposes and does not decide. Where the ADR records a decision on Q1 to Q16, this note
follows that decision and marks the row.

Every `path:line` was read at `2dc0d73c2`. Paths are shortened as in
[`docs/surfaces.md`](../surfaces.md): `core` is `crates/purlis-core/src`, `tauri` is
`app/src-tauri/src`, `ui` is `app/src`, `pf` is `docs/plane-format.md`.

## 1. Private records that move onto the record store

**What a row means.** *Today* is what a record of an unknown or newer version gets now. *Under
R2* is what it gets once the store reads it:

- **refuse**: a whole-file record that reads `TooNew` or `Unplaceable` is refused, kept, and
  never written over;
- **skip**: a row store skips that row and keeps it.

*Keeps unknown keys* says whether today's write carries keys it does not know through. Under
the store, every write keeps them.

The *Today* column uses six words: **refused** (refused whole and never overwritten),
**skipped** (skipped and left on disk), **deleted**, **overwritten** (read as empty or default,
or downgraded, and then written over), **unread** (a version is written and never compared) and
**none** (the record has no version, and so reads as unstamped version 1 under the store).
*Lock* is the read-modify-write's lock today: `flock` (cross-process), `mutex` (in-process
only) or **none**. A ⚠ marks a store that loses data today when its file is damaged or unknown.

**Approval records** (ticket 4 moves these first):

| # | File | Module | Tier | Shape | Today | Under R2 | Keeps unknown keys | Lock |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | `<config>/extensions.json` | `core/extension.rs:203` | Machine, device-bound (`pf:8817`) | whole | refused (`:2473`) | refuse | no | none (`:2624`) |
| 2 | `<config>/artifact-approvals.json` | `core/artifact/approval.rs:35` | Machine, device-bound (`pf:8813`) | whole | unread (`:432`) | refuse | no | none |
| 3 | `<config>/artifact-regenerate-approvals.json` | `core/artifact/regenerate.rs:49` | Machine, device-bound (`pf:8814`) | whole | unread (`:495`) | refuse | no | none |
| 4 | `.charter/harness-declarations-approved.json` ⚠ | `core/harness_declaration.rs:54` | Clone state (`pf:1124`) | whole | none; a bad file reads `{}` and the next approval writes only itself | refuse | while it parses | none |
| 5 | `.charter/harness-profiles-launched.json` ⚠ | `core/profiletrust.rs:26` | Clone state (`pf:1010`) | whole | as 4 | refuse | while it parses | none |
| 6 | `.charter/mcp-approved.json` ⚠ | `core/personaverbs/mcp.rs:32` | Clone state (`pf:4806`, listed again at `pf:8158`) | whole | none; as 4 | refuse; fifth adapter (ADR Q16) | while it parses | none |
| 7 | `<config>/artifact-link-hosts.json` | `core/artifact/links.rs:32` | Machine, device-bound (`pf:8816`) | whole | unread | refuse | no | none |

**Rows** (ticket 10):

| # | File | Module | Tier | Today | Under R2 | Lock |
| --- | --- | --- | --- | --- | --- | --- |
| 8 | `.charter/app/spool/messages/<chat>/<seq>.json` | `core/dispatchqueue.rs:49` | Machine, device-bound (`pf:7928`; no heading of its own) | **deleted** (`:625`; `hookwire/relay/carrying.rs:216`) | skip, listed as waiting (ADR §2); a bad MAC is moved aside into `refused/` (ADR Q15) | none (made once) |
| 9 | `.charter/app/dispatches/<ULID>.json` | `core/dispatchrecord.rs:73` | Clone state (`pf:7392`) | skipped | skip | flock |
| 10 | `.charter/app/spool/<chat>/<key>-<seq>` | `core/hookwire/spool.rs:96` | Clone state, transient (`pf:7829`) | unread; an unparseable line is deleted | skip (gap 7) | none (made once) |
| 11 | `…/skill-grades/<YYYY-MM>.jsonl` and `routers/` | `core/skills/grades.rs:35`, `core/skills/grades/routers.rs:39` | Machine, device-bound (`pf:9029`, `:9038`) | skipped | skip | none (append) |
| 12 | `…/skill-grade-resume/<grade>.json` | `core/skills/grades/resumable.rs:40` | Machine, device-bound, transient (`pf:9152`) | skipped | skip | none |
| 13 | `<data>/events/<device>/events.jsonl` | `core/eventlog.rs:79` | Machine, device-bound (`pf:8841`) | `v` read, never checked | skip | flock |
| 14 | `<config>/kill-switch.jsonl` | `core/halt.rs:35` | Machine, device-bound (`pf:8804`) | none; a bad line is skipped | skip | none |
| 15 | `<config home>/runner-hours.jsonl` | `core/runnerhours.rs:32` | Machine, device-bound (`pf:8874`) | none; a bad line is skipped and dropped on rewrite | skip | flock |
| 16 | `<data>/network/<key>.jsonl` | `core/sandboxblock_record.rs:46` | Machine, device-bound (`pf:8842`) | none; skipped and kept | skip | flock only with a config home |
| 17 | `.charter/save-journal.jsonl` | `core/planegit.rs:3383` | Clone state (`pf:8417`) | none; skipped and kept | skip | none |
| 18 | `.charter/app/arrived/<chat>.json` | `core/sandbox/arrived.rs:407` | Clone state, rebuildable (`pf:7531`) | overwritten (stamped 1) | rebuild over (ADR Q2) | flock |
| 19 | `…/skill-levels/<chat>.json` | `core/skills/levels.rs:26` | Machine, device-bound, transient (`pf:9256`) | none; replaced | rebuild over (ADR Q2) | none |
| 20 | `<config>/forge-budget/<account>.json` | `core/forge/budget.rs:236` | Machine, device-bound, transient (`pf:8819`) | none; overwritten | rebuild over (ADR Q2) | flock |
| 21 | `<config>/forge-etags/…/<sha>.json` | `core/forge/etag.rs:69` | Machine, device-bound, rebuildable (`pf:8818`) | none; replaced | rebuild over (ADR Q2) | none |

**Whole-file records** (ticket 11):

| # | File | Module | Tier | Today | Under R2 | Keeps unknown keys | Lock |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 22 | `<config>/machine.json` | `core/machine.rs:171` | Machine, device-bound (`pf:8791`) | overwritten, after a copy to `.unread` (`:2561`, `keep_unread`) | refuse; `.unread` stays for a damaged file only | no | flock |
| 23 | `<config>/profiles.json` | `core/profilestore.rs:46` | Machine, device-bound (`pf:8839`) | refused on update (`:249`) | refuse | yes | flock |
| 24 | `<config>/forge-accounts.json` | `core/forge/accounts.rs:62` | Machine, device-bound (`pf:8838`) | **overwritten** (stamped 1, `:689`) | refuse (R2) | yes | mutex |
| 25 | `<config>/onboarding.json` | `core/onboarding.rs:37` | Machine, device-bound (`pf:8797`) | overwritten | refuse | no | none |
| 26 | `<config>/layout.json` | `core/windowprefs.rs:63` | Machine, syncable (`pf:8795`) | refused on read, overwritten by a whole write | refuse | partly | flock |
| 27 | `<config>/runner-log-cursors.json` | `core/runnercursors.rs:29` | Machine, device-bound, rebuildable (`pf:8794`) | overwritten | rebuild over (ADR Q2) | no | mutex |
| 28 | `<config>/theme-choices.json` | `core/theme/choices.rs:69` | **not in `docs/plane-format.md`**: add it first | refused on edit | refuse | no | flock |
| 29 | `<config>/reports.json` | `core/report/consent.rs:35` | Machine, device-bound (`pf:8798`) | overwritten | refuse | no | none |
| 30 | `<config>/report-drafts.json` | `core/report/held.rs:99` | Machine, device-bound (`pf:8800`) | overwritten | refuse | no | mutex |
| 31 | `<config>/reports-sent.json` | `core/report/sent.rs:33` | Machine, device-bound (`pf:8799`) | overwritten | refuse | no | none |
| 32 | `<config>/push-remotes.json` ⚠ | `core/forge/gitcredential.rs:174` | Machine, device-bound (`pf:8796`) | none; one bad entry empties the map | refuse | no | none |
| 33 | `runners.json`, `runners-left.json`, `runners-unsandboxed.json`, `pool-setup.json`, `peers.json` | `core/runners.rs:47`, `runners/left.rs:29`, `runnertaint.rs:25`, `poolsetup.rs:49`, `runnerlink.rs:38` | Machine, device-bound (`pf:8873`, `:8878`, `:8877`, `:8879`, `:8880`) | refused | refuse | refuse an unknown key today (contract test 5) | flock |
| 34 | `<config home>/charterd/window-names.json` | `core/windownames.rs:26` | Machine, device-bound (`pf:8808`) | overwritten (forced to 1) | refuse | no | flock |
| 35 | `<config home>/purlisd/runners-stopped.json` | `crates/host/src/reach.rs:53` | Machine, device-bound (`pf:8810`) | none; overwritten | refuse | no | mutex |
| 36 | `<data>/inbox/<key>.json` and `.snoozed`, `.baseline`, `.seen`, `rules.json` | `core/inboxupdates.rs:34`, `askrules.rs:37` | Machine, device-bound (`pf:8846`-`:8850`) | none; overwritten | refuse (the first is transient: ADR Q2) | no | flock |
| 37 | `.charter/app/spool/messages/records.json` | `core/dispatchtalk.rs:304` | Machine, device-bound (`pf:7928`) | overwritten (stamped 2) | refuse | no | none |
| 38 | `.charter/app/dispatches/ended.json` | `core/dispatchrecord/ended.rs:38` | Clone state, rebuildable (`pf:7420`) | overwritten | rebuild over (ADR Q2) | no | none |
| 39 | `.charter/app/dispatches/refused-while-away.json` | `core/dispatchaway.rs:86` | Clone state (`pf:7511`) | refused | refuse | no | flock |
| 40 | `.charter/app/dispatch-never.json` | `core/dispatchnever.rs:60` | Clone state (`pf:6664`) | none; a bad entry refuses the change | refuse | yes | flock |
| 41 | `.charter/app/chat-records/closed.json` ⚠ | `core/chatrecords.rs:50` | Clone state (`pf:7610`) | overwritten, earlier entries lost | refuse | partly | flock |
| 42 | `.charter/app/reopen.json` | `core/reopen.rs:49` | Clone state (`pf:6847`) | overwritten | refuse | no | none |
| 43 | `<data>/app-state/<clone>/studio-chats.json` ⚠ | `core/reopen/studio_chats.rs:28` | Machine, device-bound (`pf:9012`) | overwritten, earlier entries lost | refuse | partly | flock |
| 44 | `.charter/app/secret-asks.json` | `core/secrets/ask.rs:79` | Clone state (`pf:6818`) | overwritten (stamped 1) | refuse | no | mutex |
| 45 | `<config home>/purlis/sandbox-grants/<clone>.json` and `every-project.json` | `core/sandbox/local.rs:41`, `:1453` | Machine, device-bound (`pf:4738`) | none; overwritten | refuse | no | flock |
| 46 | `.charter/app/browsers-at-quit.json` | `core/browsersatquit.rs:26` | Clone state, rebuildable (`pf:6678`) | none; deleted on read | rebuild over (ADR Q2) | no | none |
| 47 | `.charter/app/browser-timelines/<chat>/timeline.json` | `core/browsertimeline.rs:48` | Clone state, transient (`pf:6713`) | none; a bad list deletes the chat's folder | rebuild over (ADR Q2) | no | mutex |
| 48 | `.charter/app/mirrors-offered.json` | `core/guest.rs:480` | Clone state (`pf:6746`) | none; overwritten | refuse | no | flock |
| 49 | `.charter/app/spool/keys.json` | `core/hookwire/spool.rs:101` | Clone state, transient (`pf:8014`) | refused (and the doctor checks it again, `core/doctor/spool.rs:50`) | refuse; the doctor asks the store | no | flock |
| 50 | `app/spool/messages/waits.json` (runner) ⚠ | `core/hookwire/relay/carrying.rs:88` | Machine, device-bound (`pf:7951`) | overwritten, queued messages retired | refuse | no | mutex |
| 51 | `…/skill-grade-after-edit.json`, `…/skill-earlier-names.json` | `core/skills/grades/after_edit.rs:33`, `core/skills/grades/earlier.rs:45` | Machine, device-bound (`pf:9070`, `:9098`) | overwritten | refuse | no | flock |
| 52 | `skills.local.lock` | `core/skills/source/lock.rs:26` | Clone state (`pf:1767`) | refused | refuse | no | compare-and-swap |
| 53 | `.charter/cache/skill-sources-checked.json` | `core/skills/source/checked.rs:31` | Clone state, rebuildable (`pf:8663`) | overwritten | rebuild over (ADR Q2) | no | mutex |
| 54 | `.charter/plane-push.json`, `.charter/save-branch.json` | `core/planegit.rs:277`, `planegit/prsave.rs:280` | Clone state (`pf:8184`, `:8226`) | none | refuse | no | none |
| 55 | `.charter/guard-seen.json` | `core/guardseen.rs:30` | Clone state, rebuildable, legacy (`pf:8136`) | none; rebuilt | rebuild over (ADR Q2) | no | none |
| 56 | `<config home>/signing/pushed.json`, `commit-emails.json`; `.charter/signing/waiting.json` | `core/signingsetup.rs:520`, `:620`; `signingsetup/retry.rs:109` | **not in `docs/plane-format.md`**: add them first | none; overwritten | refuse | no | none |

**Out of scope:** the Plane tier (`charter.toml`, `pins.toml`, `skills.lock`, workspace markdown,
`personas/_*/*.jsonl`, and a LIVE workspace's `work/<device>.jsonl`, whose LOCAL copy is Clone
state and moves with the rows), plain-text markers and pointer files, and
`.charter/app/sandbox.json`, which nothing outside tests reads or writes (`core/sandbox/local.rs:20`)
and whose `docs/plane-format.md` entry (`pf:6614`) should say so.

**What the inventory shows beyond versions**, for the tickets that move each store:

- **Reads that follow a link or have no bound** (about twenty stores, among them 14, 16, 17, 20,
  21, 24, 36, 37, 44 and 56) get the store's gate for free.
- **Read-modify-writes with no lock, or only an in-process mutex** (1, 2, 3, 4, 5, 6, 24, 25, 29,
  32, 42 and others) get the store's lock.
- **A damaged file that loses every other entry** (the ⚠ rows) stops doing so, because the store
  refuses a file it cannot read rather than writing over it.
- `docs/plane-format.md` is missing rows 28 and 56, has no heading for row 8, and lists row 6
  twice. Each is fixed in the ticket that moves the store.

## 2. The approval adapters

### Extension

| | |
| --- | --- |
| Record | `<config>/extensions.json`, Machine, device-bound (`pf:8817`); `core/extension.rs:203`, `RECORD` |
| Key | the extension id; the entry holds its folder (`core/extension.rs:848`, `Entry`) |
| Fingerprint | every path and byte of the folder, plus the protocol when it has a program |
| Standing today | `core/extension.rs:815`, `Standing` (`Approved`, `New`, `Changed`, `Themes`) |
| Read | `core/extension.rs:2448`, `read_record`, which already refuses a newer record and never overwrites it (`:2624`, `update`) |
| Approved | `tauri/extensions.rs:1530`, `approve_extension`; `tauri/views.rs:760`, `:1235`; all call `core/extension.rs:2715`, `approve` |
| Forgotten | `tauri/extensions.rs:1551`, `forget_extension`, calling `core/extension.rs:2791`, `forget` |
| Moved | not carried: an id from another folder is another program (`core/extension.rs:2738`, `moved_clears_reading`). ADR Q13 changes it: carried to the new folder when the fingerprint is unchanged; `moved_clears_reading` changes with it |
| Gated at use | every question re-reads the record and re-fingerprints the folder (`core/executor.rs`, surfaces section 1) |
| Re-ask today | `ui/ApproveExtension.tsx:59` (a `Dialog`, "Trust it anyway" at `:73`); the row in `ui/Extensions.tsx` |
| Moves to | the code-approval Notice; the Extensions row keeps its state word |

### Page and Regenerate: one subject

| | |
| --- | --- |
| Records | page: `<config>/artifact-approvals.json` (`pf:8813`; `core/artifact/approval.rs:35`, `RECORD`). Regenerate: `<config>/artifact-regenerate-approvals.json` (`pf:8814`; `core/artifact/regenerate.rs:49`, `RECORD`). Both Machine, device-bound, and both stay where and as they are |
| Key | `core/artifact/approval.rs:106`, `key`: 16 bytes of a SHA-256 of the workspace's real path and the folder (`core/artifact/serve.rs:217`, `origin_host`); the Regenerate record reads the same key (`core/artifact/regenerate.rs:455`) |
| Fingerprint | page: the template, every path and byte but its data, handed back as served; Regenerate: the code folder and the command words (`core/artifact/regenerate.rs:275`, `fingerprint`); new: the page's `reach` (ADR §6) |
| Standing today | page `core/artifact/approval.rs:59`; Regenerate `core/artifact/regenerate.rs:382` |
| Read | page `core/artifact/approval.rs:208`, `entries`, which drops malformed entries (`:233`, `approval_of`); Regenerate `core/artifact/regenerate.rs:416`, `entries`, which does not |
| Write | page `core/artifact/approval.rs:426`, `change`; Regenerate `core/artifact/regenerate.rs:474`, `approve`. Neither holds a lock, both stamp `version: 1` and drop unknown top-level keys |
| Approved | page: `tauri/artifacts.rs:1400`, `artifact_approve` (also `grant_plane`, `core/artifact/approval.rs:331`); Regenerate: `tauri/regenerate.rs:168`, `artifact_regenerate_approve` |
| Forgotten | `core/artifact/mod.rs:428`, `unpin_here`, which calls page `forget` only (`:436`). Under the ADR it forgets both |
| Moved | `core/wscmd/rename.rs:758` calls `follow_in_approvals` (`:1292`), which calls page `moved` only (`:1298`). Under the ADR it moves both |
| Removed | `core/wscmd/remove.rs` (workspace remove) touches neither today. ADR Q11: it forgets both |
| Gated at use | page: every load and every ask on the port (`tauri/artifacts.rs:447`, `:654`, `:1012`; `tauri/artifacts/calls.rs:88`, `:242`); Regenerate: `tauri/regenerate.rs:232`, `run_pressed` (its standing at `:243`), and `:118`, `artifact_regenerate_about` (`:146`). The new page gate goes in both |
| CLI | `crates/purlis-cli/src/artifact.rs:438` (`regenerate --check`); `purlis artifact approve-status` (`core/artifact/tools.rs:185`) |
| Re-ask today | page: `ui/ArtifactTab.tsx:236`, `ScriptStanding`, and `:264`, `standingSaid`; Regenerate: `ui/ArtifactRegenerate.tsx:144` (`AlertDialog`) and `:165`, `asksBecause` |
| Moves to | one code-approval Notice per subject, in the artifact tab |

### Harness declaration, with the local profile

| | |
| --- | --- |
| Records | `.charter/harness-declarations-approved.json` (`pf:1124`; `core/harness_declaration.rs:54`, `APPROVED`) and `.charter/harness-profiles-launched.json` (`pf:1010`; `core/profiletrust.rs:26`, `RECORD`). Both Clone state, and both stay there (ADR 0069, R3) |
| Key | the declaration's name; the profile's name |
| Fingerprint | the declaration's whole digest; the profile's kind, command and environment as declared (`core/profiletrust.rs:262`, `fingerprint`) |
| Standing today | `core/profiletrust.rs:244`, `Approval` (`New`, `Changed`); approved is the absence of a value |
| Read | `core/profiletrust.rs:133`, `read_record`: no version field, and a malformed or missing file reads as empty |
| Write | `core/profiletrust.rs:167`, `record`: keeps other entries; no lock |
| Approved | `tauri/lib.rs:1865`, `approve_profile`, calling `core/profiletrust.rs:351`, `approve` |
| Forgotten | nothing today. ADR Q9: removal through purlis, or a Forget on the harness card |
| Moved | not applicable: the record is inside the clone |
| Gated at use | `core/profiletrust.rs:312`, `declaration_approval_needed_in`, from every launch (`core/personaprofile.rs:156`) |
| Re-ask today | `ui/StartApproval.tsx:78`, `StartApproveAsk` (a dialog titled "Approve <profile>?") |
| Moves to | the code-approval Notice; a Notice only, even when a chat waits on it (ADR Q14) |

### A persona's MCP servers (ADR Q16)

| | |
| --- | --- |
| Record | `.charter/mcp-approved.json`, Clone state (`pf:4806`); `core/personaverbs/mcp.rs:32`, `APPROVED_FILE` |
| Key | the persona's name; the value is the set of approved fingerprints |
| Fingerprint | the SHA-256 of each server's consent line (`core/personaverbs/mcp.rs:305`, `fingerprint`) |
| Read | `core/personaverbs/mcp.rs:355`, `approved`: no version, no bound, follows a link |
| Approved | `core/personaverbs/mcp.rs:374`, `approve`, which replaces the persona's set; no lock, and a bad file loses every other persona's |
| Forgotten, moved | nothing today |
| Moves to | the record store in any case (row 6 above); and a fifth ledger adapter, after the four (ADR Q16) |

### Out of the ledger, for the record

| Store | Why it is out | Still on the record store? |
| --- | --- | --- |
| `<config>/artifact-link-hosts.json` (`pf:8816`) | reach to a host, not code | yes, for its versioned read |
| dispatch grants: `<config home>/purlis/sandbox-grants/<clone>.json`, `[dispatch.grants]`, `.charter/app/dispatch-never.json` | reach between personas, not code; drawn by `ui/DispatchGrantNotice.tsx:169` | the two private files yes; `[dispatch.grants]` is the Plane's |
| the harness's own allow list, `PRE_ALLOWED` | the harness's store, not purlis's | no |

## 3. Contract tests that must exist first

Each test pins today's behaviour, or the behaviour R2 and R3 require, before the code under it
moves. A test is trusted only once it has been seen to fail for the right reason, so each
ticket first shows its test red against a deliberately broken store.

**The record store, for every declared store:**

1. *A record of a newer version is never overwritten.* Write a record at `VERSION + 1`, run every
   write path of that store, and assert the bytes are unchanged and the write refused. For a row
   store, assert the newer row is still there, in its place, after a rewrite.
2. *A newer whole-file record is refused, naming what is known.* The refusal names the store,
   the path, the record's version and, where the store declares it, the release that introduced
   that version (ADR Q1: no `written_by` key).
3. *A garbage version is not a missing one* (gap 2): `"8"`, `7.5` and `-1` are `Unplaceable`,
   with their own sentence.
4. *An unknown key survives a write*, at the top level and in an entry.
5. *Today's reader tolerates `reach` and any unknown key.* Run each store's reader as it is on
   main against a record carrying an extra key, and assert it reads the same as without it. This
   must be green **before** `reach` is written (ADR, What it costs).
6. *Every store the record store declares is in `docs/plane-format.md`* with its tier, beside
   the existing `every_store_the_plane_format_names_has_a_tier.rs`.
7. *Two writers do not lose each other's change*: two threads, then two processes, each
   updating a different entry, both entries kept.
8. *A link, an oversize file and a non-file are Unreadable*, never Absent, and never written
   through.

**Pinning today's answers that R2 changes** (red first, then green once the store moves):

9. The dispatch queue: a message of `v: 2` stays on disk and is listed as waiting
   (`core/dispatchqueue.rs`; today removed, surfaces section 12, UNTESTED).
10. `forge-accounts.json` at version 2 is refused and kept (`core/forge/accounts.rs`; today
    stamped back to 1, UNTESTED).
11. Spool `v` other than 1 and `keys.json` at version 3 (`core/hookwire/spool.rs`; UNTESTED).
12. The page and Regenerate records at version 2 are refused, nothing stands Approved, and
    approving refuses (today the version is never read).

**The ledger:**

13. *Unpinning forgets both parts* (gap 19's UNTESTED Regenerate unpin case). Approve the page
    and its Regenerate, unpin, pin again with the same id, and assert both ask.
14. *A rename carries both parts* (gap 19's rename case). Approve both, rename the workspace,
    and assert both stand Approved at the new path.
15. *Regenerate refuses for a page that is not Approved* (gap 16): a `New`, `Changed` and
    `PinChanged` page each refuse the run in `run_pressed` and disable it in
    `artifact_regenerate_about`; a script-free page follows ADR Q4.
16. *A Regenerate part with no page part reads as New* (ADR §10).
17. *A widened reach asks again* (gap 17): approve a page declaring `purlis.todo_list@1`, change
    the catalogue in a test build so the query is `plane`-scoped or gains a widening parameter,
    and assert `Changed(reach)`, a script-free render, and `not-approved` on the port. Then the
    same with an extension whose manifest widens its query.
18. *The plane grant does not survive a reach change.*
19. *A half-done forget fails closed*: remove the page part only, and assert Regenerate is
    refused.
20. *Every adapter answers the same four events*: a table-driven test over the four adapters,
    each declaring what it does for approve, change, forget and move, so a new adapter cannot
    leave one out.
21. *One drawing*: a window test that each adapter's `New` and `Changed` render the
    code-approval Notice with its cause key, and a guard test, beside
    `ui/Notice.guard.test.ts`, that `ApproveExtension`'s re-ask, `ScriptStanding`, the
    Regenerate `AlertDialog` and `StartApproveAsk` are gone.
22. *A re-install of a changed extension shows the re-ask*, not the first ask (gap 14).

## 4. Proposed order of tickets

Each ticket is one pull request with its tests, its `docs/plane-format.md` change first where
it writes a new key, and a changelog fragment where a person would notice.

1. **Contract tests 1 to 8 as a harness.** A test module that takes a store declaration and runs
   the record-store contract against it. It lands red-skipped for every store, so the next
   tickets switch each on.
2. **Test 5: today's readers tolerate an unknown key.** Fix any store that refuses an unknown
   key. No new key is written yet.
3. **The record store module**: the declaration, `read` built on a general `compat::Schema`,
   `update` with the lock, the kept unknown keys, and the private write. No store uses it yet.
   `docs/plane-format.md` gains each store's declared version history, under the tiers section.
4. **Move the approval records first**: `artifact-approvals.json`,
   `artifact-regenerate-approvals.json`, `extensions.json`, and the two `.charter/` consent
   records. Tests 12 and 1 to 8 go green for them. The version is now read.
5. **The ledger interface and the four adapters**, with no change of behaviour: test 20 green,
   and every existing approval test still green.
6. **R3's lifecycle**: unpin forgets both, a rename carries both, a lone Regenerate part reads
   New, and the two-file lock. Tests 13, 14, 16 and 19.
7. **R3's gate**: Regenerate refuses unless the page is Approved. Test 15 and a changelog
   fragment.
8. **R4, gap 17: the reach**, recorded on approval and checked at load and at every ask, with
   the plane grant bound to it, and `docs/plane-format.md` gaining `reach`. Tests 17 and 18;
   ADR Q6's baseline.
9. **R4, gap 14: the code-approval Notice**, its story with keyboard, axe and screenshot, and
   the four old drawings removed. Tests 21 and 22.
10. **Move the row stores**: the dispatch queue (test 9), the dispatch record, the skill grades,
    the spool (test 11), and the other JSONL logs.
11. **Move the remaining whole-file stores**: `forge-accounts.json` (test 10), the machine store,
    the reopen record, and the rest of section 1, one or a few per pull request.
12. **Remove `machine::write_beside` and `plane::write_private`** once nothing calls them, and the
    doctor's copy of the `keys.json` check.
13. **The amendments** to ADRs 0122, 0069, 0112, 0053, 0073 and 0034, as files of their own.

Tickets 1 to 3 have no behaviour a person sees. Ticket 4 is the first that could refuse a
person's record, and only one written by a newer purlis. Tickets 6 to 9 are the ones a person
notices.
