# The record store and the approval ledger: call sites, contract tests and tickets

This note is the working half of
[ADR 0125](../adr/0125-one-private-record-store-with-a-versioned-read-and-one-approval-ledger-for-code-that-runs.md),
for epic #6677. The ADR decides. This note lists what moves: every private record that would
move onto the record store, every approval adapter with its record, key and lifecycle callers,
the contract tests that must exist before anything moves, and a proposed order of tickets. It
proposes and does not decide. Where the ADR records a decision (Q1 to Q16, and R6 to R11 from
review round 1), this note follows it and marks the row. [Section 6](#6-review-round-1) says
where each finding of review round 1 went.

Every `path:line` was read at `2dc0d73c2`. Paths are shortened as in
[`docs/surfaces.md`](../surfaces.md): `core` is `crates/purlis-core/src`, `tauri` is
`app/src-tauri/src`, `ui` is `app/src`, `pf` is `docs/plane-format.md`.

## 1. Private records that move onto the record store

**What a row means.** *Today* is what a record of an unknown or newer version gets now. *Under
the store* is what it gets once the store reads it:

- **refuse**: a whole-file record that reads `TooNew` or `Unplaceable` is refused, kept, and
  never written over, with Set it aside and start fresh as its remedy (R6). A damaged one is
  copied aside and the store starts fresh (R6);
- **skip**: a row store skips that row and keeps it;
- **rebuild over**: a store declared rebuildable may be rebuilt over a newer one (R9). Only
  rebuildable stores carry it. A transient store refuses or skips like any other.

The *Today* column uses six words: **refused** (refused whole and never overwritten),
**skipped** (skipped and left on disk), **deleted**, **overwritten** (read as empty or default,
or downgraded, and then written over), **unread** (a version is written and never compared) and
**none** (the record has no version, and so reads as unstamped version 1 under the store).

*Fail closed* is what the feature does while its store refuses (D1): it may only ask more or do
less. A ✱ marks a **lockout**, a store whose refusal stops the person from something until it
is set aside or purlis is updated: for these, Set it aside and start fresh is the remedy that
matters, and test 25 pins the answer. For the other rows the answer is proposed here, and the
ticket that moves the store pins it.

*Keeps unknown keys* says whether today's write carries keys it does not know through. Under
the store, a store keeps them where its declaration says so. *Lock* is the read-modify-write's
lock today: `flock` (cross-process), `mutex` (in-process only), **none**, or a lock of the
store's own. A ⚠ marks a store that loses data today when its file is damaged or unknown.

### The declaration's fields

Each store's declaration holds these (C4, ADR §1). Tickets 3, 13 and 14 fill them in per store,
and most stores are rewrites, not switches, because these differ between them:

| Field | Values met in the inventory |
| --- | --- |
| Root, apart from the tier | the config home, `<data>`, `<config home>`, the session host's folder, the plane's `.charter/`, and `hookstate::trust_root` (`core/personaverbs/mcp.rs:390`, for row 6) |
| Tier and marks | as `pf`'s `**Tier:**` line; **rebuildable** decides *rebuild over* (R9) |
| Shape | whole file; rows appended to JSONL; one file per record, made once |
| Version key, own, oldest, migrations | `version`, `v`, or none (unstamped 1); TooOld's migration defaults to the damage rule (D3) |
| Bound | today's, moved into the declaration |
| Lock | blocking named lock (`core/machine.rs:2640`, `Lock::named`); `rewrite::Lock::bounded` (`core/rewrite.rs:209`); the event log's own; the compare-and-swap of `skills.local.lock`; none today, one added |
| Busy sentence | the store's own; an approval's is "purlis is still recording another approval. Nothing was approved. Try again." |
| Serializer | `pyjson::dumps_indent2` (rows 2, 3), `pyjson::dumps_indent2_unicode` (row 6), sorted, serde pretty. Each ticket reads its store's and keeps it, so no file's bytes change |
| Keeps unknown keys | at which levels; about 45 of the 56 stores do not today |
| Must not be tracked | rows 4, 5 and 6 (B5): a tracked or un-ignored record approves nothing |
| Fail-closed answer | the column below |
| Dispatch MAC | row 8 checks its MAC over the exact bytes written, so its serializer is fixed by the MAC, not chosen |

**Approval records** (ticket 3 moves rows 1 to 5; ticket 12 moves row 6; row 7 moves with the
whole-file stores in ticket 14):

| # | File | Module | Tier | Shape | Today | Under the store | Fail closed | Keeps unknown keys | Lock |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | `<config>/extensions.json` | `core/extension.rs:203` | Machine, device-bound (`pf:8817`) | whole | refused (`:2473`) | refuse | ✱ no extension stands approved, so none runs; approving refuses | no | none (`:2624`) |
| 2 | `<config>/artifact-approvals.json` ⚠ | `core/artifact/approval.rs:35` | Machine, device-bound (`pf:8813`) | whole | unread (`:432`); a bad file reads as empty and the next approval writes only itself | refuse | ✱ no page's script runs; approving refuses | entries yes, top level no | none |
| 3 | `<config>/artifact-regenerate-approvals.json` ⚠ | `core/artifact/regenerate.rs:49` | Machine, device-bound (`pf:8814`) | whole | unread (`:495`); as 2 | refuse | ✱ no Regenerate runs; approving refuses | entries yes, top level no | none |
| 4 | `.charter/harness-declarations-approved.json` ⚠ | `core/harness_declaration.rs:54` | Clone state (`pf:1124`) | whole | none; a bad file reads `{}` and the next approval writes only itself | refuse | ✱ no declaration stands approved; a chat on one is held, as a Blocking ask (R7) | while it parses | none |
| 5 | `.charter/harness-profiles-launched.json` ⚠ | `core/profiletrust.rs:26` | Clone state (`pf:1010`) | whole | as 4 | refuse | ✱ as 4, for local profiles | while it parses | none |
| 6 | `.charter/mcp-approved.json` ⚠ | `core/personaverbs/mcp.rs:32` | Clone state (`pf:4806`, listed again at `pf:8158`) | whole | none; as 4 | refuse; fifth adapter (Q16, R11) | every credentialed server is withheld from chats | while it parses | none |
| 7 | `<config>/artifact-link-hosts.json` | `core/artifact/links.rs:32` | Machine, device-bound (`pf:8816`) | whole | unread | refuse | no host stands allowed, so each link asks | no | none |

**Rows** (ticket 13). A row store never locks out: only the row it cannot place is left
unread.

| # | File | Module | Tier | Today | Under the store | Fail closed | Lock |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 8 | `.charter/app/spool/messages/<chat>/<seq>.json` | `core/dispatchqueue.rs:49` | Machine, device-bound (`pf:7928`; no heading of its own) | **deleted** (`:625`; `hookwire/relay/carrying.rs:402`) | skip, listed as waiting (ADR §2); the floor never passes it (D5); a bad MAC is moved aside into `refused/` (Q15), a missing key leaves it waiting (D4) | the message waits | none (made once) |
| 9 | `.charter/app/dispatches/<ULID>.json` | `core/dispatchrecord.rs:73` | Clone state (`pf:7392`) | skipped | skip | that dispatch is not listed; the doctor counts it | flock |
| 10 | `.charter/app/spool/<chat>/<key>-<seq>` | `core/hookwire/spool.rs:96` | Clone state, transient (`pf:7829`) | unread; an unparseable line is deleted | skip (gap 7) | the line waits, undelivered | none (made once) |
| 11 | `…/skill-grades/<YYYY-MM>.jsonl` and `routers/` | `core/skills/grades.rs:35`, `core/skills/grades/routers.rs:39` | Machine, device-bound (`pf:9029`, `:9038`) | skipped | skip | that grade is not counted | none (append) |
| 12 | `…/skill-grade-resume/<grade>.json` | `core/skills/grades/resumable.rs:40` | Machine, device-bound, transient (`pf:9152`) | skipped | skip (R9: transient is not rebuilt over) | that grade is not resumed | none |
| 13 | `<data>/events/<device>/events.jsonl` | `core/eventlog.rs:79` | Machine, device-bound (`pf:8841`) | `v` read, never checked | skip | that event is not read | its own flock |
| 14 | `<config>/kill-switch.jsonl` | `core/halt.rs:35` | Machine, device-bound (`pf:8804`) | none; a bad line is skipped | skip | to decide in its ticket: a line this build cannot read may be a halt, so doing less may mean halting | none |
| 15 | `<config home>/runner-hours.jsonl` | `core/runnerhours.rs:32` | Machine, device-bound (`pf:8874`) | none; a bad line is skipped and dropped on rewrite | skip; never pruned (`:258`, `pruned`) or dropped for the bound (D6) | that line's hours are not counted | flock |
| 16 | `<data>/network/<key>.jsonl` | `core/sandboxblock_record.rs:46` | Machine, device-bound (`pf:8842`) | none; skipped and kept | skip | that block is not listed | flock only with a config home |
| 17 | `.charter/save-journal.jsonl` | `core/planegit.rs:3383` | Clone state (`pf:8417`) | none; skipped and kept | skip | that entry is not replayed | none |
| 18 | `.charter/app/arrived/<chat>.json` | `core/sandbox/arrived.rs:407` | Clone state, rebuildable (`pf:7531`) | overwritten (stamped 1) | rebuild over (R9) | rebuilt | flock |
| 19 | `…/skill-levels/<chat>.json` | `core/skills/levels.rs:26` | Machine, device-bound, transient (`pf:9256`) | none; replaced | refuse (R9) | no level kept for that chat | none |
| 20 | `<config>/forge-budget/<account>.json` | `core/forge/budget.rs:236` | Machine, device-bound, transient (`pf:8819`) | none; overwritten | refuse (R9) | that account's budget is unknown; its ticket decides whether that waits as a spent budget | flock |
| 21 | `<config>/forge-etags/…/<sha>.json` | `core/forge/etag.rs:69` | Machine, device-bound, rebuildable (`pf:8818`) | none; replaced | rebuild over (R9) | rebuilt | none |

**Whole-file records** (ticket 14):

| # | File | Module | Tier | Today | Under the store | Fail closed | Keeps unknown keys | Lock |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 22 | `<config>/machine.json` | `core/machine.rs:171` | Machine, device-bound (`pf:8791`) | another version, or a file that does not parse, is copied to `.unread` and overwritten (`:2561`, `update`; `:2595`, `keep_unread`); a file it cannot read at all is never overwritten | refuse a newer one (R2), with Set it aside and start fresh; a damaged one is copied aside, never replacing an earlier copy, and starts fresh (R6) | ✱ no plane, approval of a plane or pool is known; a chat in a workspace with no place chosen is refused, as for an unreadable file today | no | blocking named lock (`:2640`) |
| 23 | `<config>/profiles.json` | `core/profilestore.rs:46` | Machine, device-bound (`pf:8839`) | refused on update (`:249`) | refuse | ✱ no harness profile of this machine is known | yes | flock |
| 24 | `<config>/forge-accounts.json` | `core/forge/accounts.rs:62` | Machine, device-bound (`pf:8838`) | **overwritten** (stamped 1, `:689`) | refuse (R2) | no forge account is known; signing in again refuses | yes | mutex |
| 25 | `<config>/onboarding.json` | `core/onboarding.rs:37` | Machine, device-bound (`pf:8797`) | overwritten | refuse | no progress is known; the steps start again and are not kept | no | none |
| 26 | `<config>/layout.json` | `core/windowprefs.rs:63` | Machine, syncable (`pf:8795`) | refused on read, overwritten by a whole write | refuse | the defaults, not kept | partly | flock |
| 27 | `<config>/runner-log-cursors.json` | `core/runnercursors.rs:29` | Machine, device-bound, rebuildable (`pf:8794`) | overwritten | rebuild over (R9) | rebuilt from the log's end | no | mutex |
| 28 | `<config>/theme-choices.json` | `core/theme/choices.rs:69` | **not in `docs/plane-format.md`**: add it first | refused on edit | refuse | the default theme, not kept | no | flock |
| 29 | `<config>/reports.json` | `core/report/consent.rs:35` | Machine, device-bound (`pf:8798`) | overwritten | refuse | nothing answered, so nothing is sent | no | none |
| 30 | `<config>/report-drafts.json` | `core/report/held.rs:99` | Machine, device-bound (`pf:8800`) | overwritten | refuse | no draft is held or sent | no | mutex |
| 31 | `<config>/reports-sent.json` | `core/report/sent.rs:33` | Machine, device-bound (`pf:8799`) | overwritten | refuse | nothing waiting is sent | no | none |
| 32 | `<config>/push-remotes.json` ⚠ | `core/forge/gitcredential.rs:174` | Machine, device-bound (`pf:8796`) | none; one bad entry empties the map | refuse | no remote is recorded, so an automatic save takes the forge CLI's helper, as for a missing file | no | none |
| 33 | `runners.json`, `runners-left.json`, `runners-unsandboxed.json`, `pool-setup.json`, `peers.json` | `core/runners.rs:47`, `runners/left.rs:29`, `runnertaint.rs:25`, `poolsetup.rs:49`, `runnerlink.rs:38` | Machine, device-bound (`pf:8873`, `:8878`, `:8877`, `:8879`, `:8880`) | refused | refuse | no pool or runner is known, so no chat starts on one | refuse an unknown key today (test 5) | flock |
| 34 | `<config home>/charterd/window-names.json` | `core/windownames.rs:26` | Machine, device-bound (`pf:8808`) | overwritten (forced to 1) | refuse | windows keep their default names | no | flock |
| 35 | `<config home>/purlisd/runners-stopped.json` | `crates/host/src/reach.rs:53` | Machine, device-bound (`pf:8810`) | none; overwritten | refuse | no runner is known as stopped | no | mutex |
| 36 | `<data>/inbox/<key>.json` and `.snoozed`, `.baseline`, `.seen`, `rules.json` | `core/inboxupdates.rs:34`, `askrules.rs:37` | Machine, device-bound (`pf:8846`-`:8850`); the first is transient | none; overwritten | refuse (R9: the transient one too) | no update, snooze or rule is known | no | flock |
| 37 | `.charter/app/spool/messages/records.json` | `core/dispatchtalk.rs:304` | Machine, device-bound (`pf:7928`) | overwritten (stamped 2) | refuse | no record of a message is shown | no | none |
| 38 | `.charter/app/dispatches/ended.json` | `core/dispatchrecord/ended.rs:38` | Clone state, rebuildable (`pf:7420`) | overwritten | rebuild over (R9) | rebuilt | no | none |
| 39 | `.charter/app/dispatches/refused-while-away.json` | `core/dispatchaway.rs:86` | Clone state (`pf:7511`) | refused | refuse | nothing is shown as blocked while away | no | flock |
| 40 | `.charter/app/dispatch-never.json` | `core/dispatchnever.rs:60` | Clone state (`pf:6664`) | none; a bad entry refuses the change | refuse | every pair counts as never, so no dispatch goes | yes | flock |
| 41 | `.charter/app/chat-records/closed.json` ⚠ | `core/chatrecords.rs:50` | Clone state (`pf:7610`) | overwritten, earlier entries lost | refuse | no closed chat is listed | partly | flock |
| 42 | `.charter/app/reopen.json` | `core/reopen.rs:49` | Clone state (`pf:6847`) | overwritten | refuse | nothing reopens | no | none |
| 43 | `<data>/app-state/<clone>/studio-chats.json` ⚠ | `core/reopen/studio_chats.rs:28` | Machine, device-bound (`pf:9012`) | overwritten, earlier entries lost | refuse | no studio chat reopens | partly | flock |
| 44 | `.charter/app/secret-asks.json` | `core/secrets/ask.rs:79` | Clone state (`pf:6818`) | overwritten (stamped 1) | refuse | ✱ no secret ask is answered or made | no | mutex |
| 45 | `<config home>/purlis/sandbox-grants/<clone>.json` and `every-project.json` | `core/sandbox/local.rs:41`, `:1453` | Machine, device-bound (`pf:4738`) | none; overwritten | refuse | no grant of this machine counts | no | flock |
| 46 | `.charter/app/browsers-at-quit.json` | `core/browsersatquit.rs:26` | Clone state, rebuildable (`pf:6678`) | none; deleted on read | rebuild over (R9) | no browser comes back | no | none |
| 47 | `.charter/app/browser-timelines/<chat>/timeline.json` | `core/browsertimeline.rs:48` | Clone state, transient (`pf:6713`) | none; a bad list deletes the chat's folder | refuse (R9) | that chat's timeline is not shown or added to | no | mutex |
| 48 | `.charter/app/mirrors-offered.json` | `core/guest.rs:480` | Clone state (`pf:6746`) | none; overwritten | refuse | no mirror is offered | no | flock |
| 49 | `.charter/app/spool/keys.json` | `core/hookwire/spool.rs:101` | Clone state, transient (`pf:8014`) | refused (and the doctor checks it again, `core/doctor/spool.rs:50`) | refuse (R9: never rebuilt over, it holds the replay floors); the doctor asks the store | ✱ no message is delivered, and none is moved to `refused/` (D4) | no | flock |
| 50 | `app/spool/messages/waits.json` (runner) ⚠ | `core/hookwire/relay/carrying.rs:88` | Machine, device-bound (`pf:7951`) | overwritten, queued messages retired | refuse | no message is retired; they wait | no | mutex |
| 51 | `…/skill-grade-after-edit.json`, `…/skill-earlier-names.json` | `core/skills/grades/after_edit.rs:33`, `core/skills/grades/earlier.rs:45` | Machine, device-bound (`pf:9070`, `:9098`) | overwritten | refuse | not kept | no | flock |
| 52 | `skills.local.lock` | `core/skills/source/lock.rs:26` | Clone state (`pf:1767`) | refused | refuse | no local skill source is changed | no | compare-and-swap |
| 53 | `.charter/cache/skill-sources-checked.json` | `core/skills/source/checked.rs:31` | Clone state, rebuildable (`pf:8663`) | overwritten | rebuild over (R9) | rebuilt | no | mutex |
| 54 | `.charter/plane-push.json`, `.charter/save-branch.json` | `core/planegit.rs:277`, `planegit/prsave.rs:280` | Clone state (`pf:8184`, `:8226`) | none | refuse | no automatic push or save branch | no | none |
| 55 | `.charter/guard-seen.json` | `core/guardseen.rs:30` | Clone state, rebuildable, legacy (`pf:8136`) | none; rebuilt | rebuild over (R9) | rebuilt | no | none |
| 56 | `<config home>/signing/pushed.json`, `commit-emails.json`; `.charter/signing/waiting.json` | `core/signingsetup.rs:520`, `:620`; `signingsetup/retry.rs:109` | **not in `docs/plane-format.md`**: add them first | none; overwritten | refuse | signing setup is not retried | no | none |

**New stores this effort adds**, each with its `pf` entry and tier before the code writes it
(A3, D11):

| File | Tier | Holds | Bound and cleanup |
| --- | --- | --- | --- |
| `.charter/app/spool/messages/refused/` | Machine, device-bound, as row 8 | dispatch messages whose MAC did not match (Q15), `0700`, denied to every chat's sandbox | at most 64; past that a failing message stays where it is, listed; nothing removes one but the person |
| `<file>.unread`, `<file>.unread-<UTC time>`, `<file>.set-aside-<UTC time>` beside any whole-file store | its store's | a damaged record copied aside (R6), or a record set aside on the person's press | never replaced; the doctor counts and names them; nothing removes one but the person |

**Out of scope:** the Plane tier (`charter.toml`, `pins.toml`, `skills.lock`, workspace markdown,
`personas/_*/*.jsonl`, and a LIVE workspace's `work/<device>.jsonl`, whose LOCAL copy is Clone
state and moves with the rows), plain-text markers and pointer files, and
`.charter/app/sandbox.json`, which nothing outside tests reads or writes
(`core/sandbox/local.rs:20`) and whose `docs/plane-format.md` entry (`pf:6614`) should say so.

**What the inventory shows beyond versions**, for the tickets that move each store:

- **Reads that follow a link or have no bound** (about twenty stores, among them 14, 16, 17, 20,
  21, 24, 36, 37, 44 and 56) get the store's gate for free.
- **Read-modify-writes with no lock, or only an in-process mutex** (1, 2, 3, 4, 5, 6, 24, 25, 29,
  32, 42 and others) get a lock of their own. Each store keeps its own lock, so no two stores
  wait on each other (D8), and a store that has a lock file today keeps its name, so an older
  purlis and a newer one still exclude each other.
- **A damaged file that loses every other entry** (the ⚠ rows) stops doing so. The store copies
  a damaged file aside before it starts fresh, and refuses a newer one rather than writing over
  it.
- `docs/plane-format.md` is missing rows 28 and 56, has no heading for row 8, and lists row 6
  twice. Each is fixed in the ticket that moves the store. Row 22's `.unread` entry
  (`pf:8793`) changes from "one copy, the latest" to copies never replaced.

## 2. The approval adapters

### Extension

| | |
| --- | --- |
| Record | `<config>/extensions.json`, Machine, device-bound (`pf:8817`); `core/extension.rs:203`, `RECORD` |
| Key | the extension id; the entry holds its folder (`core/extension.rs:848`, `Entry`) |
| Fingerprint | every path and byte of the folder, plus the protocol when it has a program. The state folder is left out, and a link is hashed by its target's words, never followed (`core/extension.rs:146`-`:172`) |
| Standing today | `core/extension.rs:815`, `Standing` (`Approved`, `New`, `Changed`, `Themes`) |
| Read | `core/extension.rs:2448`, `read_record`, which already refuses a newer record and never overwrites it (`:2624`, `update`) |
| A registry, not only an approval | the record also holds each extension's source, whether it is on, whether it reads prompts, and its unapproved rows; its forget uninstalls. The adapter answers through the registry's own calls, and the registry keeps owning its writes (C7) |
| Approved | `tauri/extensions.rs:1530`, `approve_extension`, which calls `core/extension.rs:2715`, `approve`, at `:1537` |
| Forgotten | `tauri/extensions.rs:1551`, `forget_extension`, calling `core/extension.rs:2791`, `forget` |
| Moved | not carried today: an id from another folder is another program (`core/extension.rs:2738`, `moved_clears_reading`). Q13, narrowed by B2: carried to the new folder only when the fingerprint is unchanged, no link in the folder resolves outside it, and its state folder at the new place is absent or empty. `moved_clears_reading` changes with it (ticket 10) |
| Gated at use | every question re-reads the record and re-fingerprints the folder (`core/executor.rs`, surfaces section 1) |
| Re-ask today | `ui/ApproveExtension.tsx:59` (a `Dialog`, "Trust it anyway" at `:73`); the row in `ui/Extensions.tsx` |
| Moves to | the code-approval Notice, carrying `RUNS_AS_YOU` and `FINGERPRINTED` (`core/extension.rs:321`, `:339`) as given (E2); the Extensions row keeps its standing word |

### Page and Regenerate: one subject

| | |
| --- | --- |
| Records | page: `<config>/artifact-approvals.json` (`pf:8813`; `core/artifact/approval.rs:35`, `RECORD`). Regenerate: `<config>/artifact-regenerate-approvals.json` (`pf:8814`; `core/artifact/regenerate.rs:49`, `RECORD`). Both Machine, device-bound, and both stay where and as they are |
| Key | `core/artifact/approval.rs:106`, `key`: 16 bytes of a SHA-256 of the workspace's real path and the folder (`core/artifact/serve.rs:217`, `origin_host`); the Regenerate record reads the same key (`core/artifact/regenerate.rs:455`) |
| Fingerprint | page: the template, every path and byte but its data, handed back as served; new: the page's `reach` (ADR §6). Regenerate: the code folder and the command words (`core/artifact/regenerate.rs:275`, `fingerprint`), which frames the folder's absolute path today (`:288`). R8: framed relative to its workspace, or to the plane for a skill's folder, with the framing version raised. New: the stamp of the template it was approved beside (B1) |
| Standing today | page `core/artifact/approval.rs:59`; Regenerate `core/artifact/regenerate.rs:382` |
| Read | page `core/artifact/approval.rs:208`, `entries`, which drops malformed entries (`:233`, `approval_of`); Regenerate `core/artifact/regenerate.rs:416`, `entries`, which does not. A file of either that does not parse reads as empty |
| Write | page `core/artifact/approval.rs:426`, `change`; Regenerate `core/artifact/regenerate.rs:474`, `approve`. Neither holds a lock, both stamp `version: 1`, keep other entries whole and drop unknown top-level keys. Under the ADR, approve replaces the subject's entry whole (B4) |
| Approved | page: `tauri/artifacts.rs:1400`, `artifact_approve` (also `grant_plane`, `core/artifact/approval.rs:331`); Regenerate: `tauri/regenerate.rs:168`, `artifact_regenerate_approve`. Q5 adds one command that approves the listed parts under one lock, Regenerate part first (D9) |
| Storage origin | every page approval draws a new one (`core/artifact/approval.rs:125`, `page_host`; ADR 0122 as amended 2026-11-10), a reach re-approval included; a Regenerate-only approval leaves it (A6) |
| Forgotten | `core/artifact/mod.rs:428`, `unpin_here`, which calls page `forget` only (`:436`). Under the ADR it forgets both, page part first |
| Moved | `core/wscmd/rename.rs:758` calls `follow_in_approvals` (`:1292`), which calls page `moved` only (`:1298`). Under the ADR it moves both, Regenerate part first, carrying unknown keys |
| Removed | `core/wscmd/remove.rs` (workspace remove) touches neither today. Q11: it forgets both (ticket 6) |
| Gated at use | page: every load and every ask on the port (`tauri/artifacts.rs:447`, `:654`, `:1012`; `tauri/artifacts/calls.rs:88`, `:242`); Regenerate: `tauri/regenerate.rs:232`, `run_pressed` (its standing at `:243`), and `:118`, `artifact_regenerate_about` (`:146`). The new page gate goes in both |
| CLI | `crates/purlis-cli/src/artifact.rs:438` (`regenerate --check`); `purlis artifact approve-status` (`core/artifact/tools.rs:185`) |
| Re-ask today | page: `ui/ArtifactTab.tsx:236`, `ScriptStanding`, and `:264`, `standingSaid`; Regenerate: `ui/ArtifactRegenerate.tsx:144` (`AlertDialog`, "Approve and run" at `:157`) and `:165`, `asksBecause` |
| Moves to | one code-approval Notice per subject, in the artifact tab's band and in Notices, with a headed line per part and "Approve script and Regenerate code" when both ask (E5) |

### Harness declaration, with the local profile

| | |
| --- | --- |
| Records | `.charter/harness-declarations-approved.json` (`pf:1124`; `core/harness_declaration.rs:54`, `APPROVED`) and `.charter/harness-profiles-launched.json` (`pf:1010`; `core/profiletrust.rs:26`, `RECORD`). Both Clone state, and both stay there (ADR 0069, R3). Both declared "must not be tracked" (B5) |
| Key | the declaration's name; the profile's name |
| Fingerprint | the declaration's whole digest; the profile's kind, command and environment as declared (`core/profiletrust.rs:262`, `fingerprint`) |
| Standing today | `core/profiletrust.rs:244`, `Approval` (`New`, `Changed`); approved is the absence of a value |
| Read | `core/profiletrust.rs:133`, `read_record`: no version field, and a malformed or missing file reads as empty |
| Write | `core/profiletrust.rs:167`, `record`: keeps other entries; no lock |
| Approved | `tauri/lib.rs:1865`, `approve_profile`, which takes the `shown` string, not a fingerprint, and calls `core/profiletrust.rs:351`, `approve`. The start dialog then starts the same chat again (`ui/StartApproval.tsx:105`, `asked.again`), so the Notice and the ask must carry that start (C3) |
| Forgotten | nothing today. Q9: removal through purlis, or a Forget on the harness card (ticket 9) |
| Moved | not applicable: the record is inside the clone |
| Gated at use | `core/profiletrust.rs:312`, `declaration_approval_needed_in`, from every launch (`core/personaprofile.rs:156`) |
| Re-ask today | `ui/StartApproval.tsx:78`, `StartApproveAsk` (a dialog titled "Approve <profile>?", "Approve and start") |
| Moves to | the code-approval Notice, and a Blocking ask in Asks while a chat is held on it, both from the one standing (R7) |

### A persona's MCP servers (Q16, R11)

| | |
| --- | --- |
| Record | `.charter/mcp-approved.json`, Clone state (`pf:4806`); `core/personaverbs/mcp.rs:32`, `APPROVED_FILE`. Declared "must not be tracked" (B5); its root is `hookstate::trust_root` |
| Key | the persona's name; the value is the set of approved fingerprints, with no server name |
| Fingerprint | the SHA-256 of each server's line as the person is shown it (`core/personaverbs/mcp.rs:305`, `fingerprint`) |
| Standing | Approved or New only (R11): a changed line cannot be told from a new one |
| Read | `core/personaverbs/mcp.rs:355`, `approved`: no version, no bound, follows a link |
| Approved | `purlis persona approve-mcp` in the person's terminal (`core/personaverbs/approve.rs:110`), calling `core/personaverbs/mcp.rs:374`, `approve`, which replaces the persona's set; no lock, and a bad file loses every other persona's |
| At use | an unapproved credentialed server is withheld from the chat (`core/personaverbs/chatstart.rs:72`, in `servers`); the chat is not held |
| Forgotten, moved | not raised, as a decision (R11) |
| Moves to | the record store (row 6); a fifth ledger adapter, after the four (ticket 12); its Notice offers Copy command (ADR N2) |

### Out of the ledger, for the record

| Store | Why it is out | Still on the record store? |
| --- | --- | --- |
| `<config>/artifact-link-hosts.json` (`pf:8816`) | access (what a page may open), not code | yes, for its versioned read |
| dispatch grants: `<config home>/purlis/sandbox-grants/<clone>.json`, `[dispatch.grants]`, `.charter/app/dispatch-never.json` | access (who may dispatch to whom), not code; drawn by `ui/DispatchGrantNotice.tsx:169` | the two private files yes; `[dispatch.grants]` is the Plane's |
| the harness's own allow list, `PRE_ALLOWED` | the harness's store, not purlis's | no |

## 3. Contract tests that must exist first

Each test pins today's behaviour, or the behaviour the ADR requires, before the code under it
moves. A test is trusted only once it has been seen to fail for the right reason, so each
ticket first shows its test red against a deliberately broken store.

**The record store, for every declared store** (run against a toy store in ticket 1, and
switched on per store as it moves):

1. *A record of a newer version is never overwritten.* Write a record at `VERSION + 1`, run every
   write path of that store, and assert the bytes are unchanged and the write refused. For a row
   store, assert the newer row is still there, in its place, after a rewrite.
2. *A newer whole-file record is refused, saying what stopped working.* The sentence leads with
   the store's name for itself and says a newer purlis wrote it; the detail names the path and
   the version found (ADR §2, E1). A CLI command that meets it exits 1 (ADR 0120, A11).
3. *A garbage version is not a missing one* (gap 2): `"8"`, `7.5`, `0` and `-1` are
   `Unplaceable`, with their own sentence, and a negative integer is never Understood (A11).
4. *An unknown key survives a write*, at the top level and in an entry the write does not
   replace.
5. *Today's reader tolerates an unknown key.* Run the store's reader as it is on main against a
   record carrying an extra key, and assert it reads the same as without it. It runs in the
   ticket that moves each store, before that store's first new key (C8).
6. *Every store the record store declares is in `docs/plane-format.md`* with its tier, beside
   the existing `every_store_the_plane_format_names_has_a_tier.rs`.
7. *Two writers do not lose each other's change*: two threads, then two processes, each
   updating a different entry, both entries kept.
8. *A link, an oversize file and a non-file are Unreadable*, never Absent, and never written
   through.
23. *A damaged whole file is copied aside, then the store starts fresh* (R6, D2): bytes that are
    not JSON read as `Damaged`; the next update leaves them byte for byte in `<file>.unread`;
    a second damage writes `<file>.unread-<time>` and leaves the first.
24. *Set it aside and start fresh* (R6, D1): for TooNew and Unplaceable, the press renames the
    file with no link followed and nothing replaced, the store reads `Absent`, and nothing is
    deleted.
25. *Each lockout store fails closed* (D1): every ✱ row, refused, does only its declared answer,
    and approving or changing refuses with the store's sentence.
26. *One Notice for every refused store* (R10): three stores refused at once raise one
    `records-unread` Notice listing all three, which settles when the last reads again.
27. *TooOld runs the declared migration* (D3): a toy store whose oldest is 2 meets version 1 and
    applies its migration, by default the damage rule.
28. *A torn last line is a skipped row* (D7), kept on disk, and the next append starts on a
    fresh line.
29. *Pruning and the bound keep skipped rows* (D6): `runner-hours.jsonl` with a newer row past
    `KEPT_FOR` keeps it through `pruned`; skipped rows alone over the bound refuse an append with
    the store's sentence.
30. *Locks* (D8): a lock file an older purlis takes (`machine.json.lock`) excludes the store's
    update; a bounded lock held past its wait refuses with the store's busy sentence, never
    `rewrite`'s chat sentence for an approval; two stores never wait on each other; where `flock`
    is unsupported the update goes ahead.
31. *Only rebuildable stores are rebuilt over a newer one* (R9, D10, B7): a transient store
    (the forge budget) and the spool's `keys.json` at a newer version are refused; a rebuildable
    one (`forge-etags`) is rebuilt.

**Pinning today's answers that the ADR changes** (red first, then green once the store moves):

9. The dispatch queue: a message of `v: 2` stays on disk and is listed as waiting
   (`core/dispatchqueue.rs`; today removed, surfaces section 12, UNTESTED).
10. `forge-accounts.json` at version 2 is refused and kept (`core/forge/accounts.rs`; today
    stamped back to 1, UNTESTED).
11. Spool `v` other than 1 and `keys.json` at version 3 (`core/hookwire/spool.rs`; UNTESTED).
12. The page and Regenerate records at version 2 are refused, nothing stands Approved, and
    approving refuses (today the version is never read).
32. *The floor never passes a skipped message* (D5): a `v: 2` message at seq 4 is kept; the
    older purlis delivers seq 5; a newer purlis then still finds seq 4 waiting, never dropped as
    delivered.
33. *A key that is unavailable leaves messages waiting* (D4): with `keys.json` refused, damaged
    or unreadable, no message moves to `refused/`.
34. *`refused/` is hardened* (B6, D11): it is made `0700`; a message is moved with no link
    followed and nothing replaced; the 65th failing message stays where it was; the listing
    shows the count, seqs and paths and no body; `chats_in` never lists it; a chat's sandbox
    profile denies it.

**The ledger:**

13. *Unpinning forgets both parts* (gap 19's UNTESTED Regenerate unpin case). Approve the page
    and its Regenerate, unpin, pin again with the same id, and assert both ask.
14. *A rename carries both parts* (gap 19's rename case), and the Regenerate fingerprint is not
    computed again (R8). Approve both, rename the workspace, and assert both stand Approved at
    the new path. A Regenerate approval in the old framing asks once.
15. *Regenerate refuses for a page that is not Approved* (gap 16): a `New`, `Changed`,
    `PinChanged` and `NotHere` page each refuse the run in `run_pressed` and disable it in
    `artifact_regenerate_about`; a script-free page passes (Q4).
16. *A Regenerate part with no page part reads as New* (ADR §10).
17. *A changed reach asks again* (gap 17): approve a page declaring `purlis.todo_list@1`, change
    the catalogue in a test build so the query is `plane`-scoped or gains a widening parameter,
    and assert `Changed(reach)`, a script-free render, and `not-approved` on the port. Then the
    same with an extension whose manifest widens its query, and with one that narrows it.
18. *The plane grant does not survive a reach change.*
19. *Every crash between two writes fails closed* (D9): fail the second write of approve, forget
    and moved in turn, and assert Regenerate is refused each time and the page asks.
20. *Every adapter answers the same four events*: a table-driven test over the adapters, each
    declaring what it does for approve, change, forget and move, so a new adapter cannot leave
    one out.
21. *One Notice*: a window test that each adapter's `New` and `Changed` render the
    code-approval Notice with its cause key, and a guard test, beside
    `ui/Notice.guard.test.ts`, that `ApproveExtension`'s re-ask, `ScriptStanding`, the
    Regenerate `AlertDialog` and `StartApproveAsk` are gone. The guard's banned list grows with
    each adapter's ticket.
22. *A re-install of a changed extension shows the re-ask*, not the first ask (gap 14).
35. *A script-free re-pin does not revive an orphaned Regenerate approval* (B1): approve a
    scripted page and its Regenerate, unpin, re-pin a script-free template with the same command
    and so the same id, and assert Regenerate stands New.
36. *A moved extension carries its approval only under Q13's conditions* (B2): a clean copy
    carries; a copy with a file in its state folder, and one with a relative link resolving
    outside the folder, each ask.
37. *Approve replaces the subject's entry whole* (B4): an unknown key on the approved entry is
    gone after approve; one on another entry, at the top level, and on an entry `moved` carries
    is kept.
38. *A tracked Clone-state approval record approves nothing* (B5): rows 4, 5 and 6, committed or
    not ignored, read as nothing approved with the sentence.
39. *A page with no `reach`* (D12, C8): compared with the baseline, it stands Approved while
    nothing it declares reads more; a query absent from the baseline asks; a declared extension
    query asks once; an approval by an older purlis, which writes no `reach`, falls back to the
    baseline.
40. *A held chat is one Blocking ask and the Notice* (R7, E3): a harness declaration changed
    while two chats wait on it raises one ask in Asks and one Notice; an answer in either
    settles both; dismissing the Notice leaves the ask; each chat's refusal names the row.
41. *The Notice says how code runs* (E2): an extension's Notice renders `RUNS_AS_YOU` and
    `FINGERPRINTED` as the core gives them, and a harness's says its command runs as you.
42. *An Approve raised by a press finishes the press* (E6, C3): "Approve and run" approves then
    runs Regenerate once; "Approve and start" approves then starts the held chat again.
43. *The MCP adapter answers only Approved or New* (R11), and its approve replaces the persona's
    set and keeps every other persona's.

## 4. Proposed order of tickets

Each ticket is one pull request with its tests, its `docs/plane-format.md` change first where
it writes a new key or store, and a changelog fragment where a person would notice. Ticket 3
and tickets 13 and 14 are per-store rewrites, sized one or a few stores per pull request.

1. **The record store module, its contract harness and a toy store, together** (C1). The
   declaration with every field of section 1, `read` with the six answers, `update` and
   `update_held` with the declared lock (C5), the kept unknown keys, the damage rule and Set it
   aside, and the private write. The harness takes a declaration and runs the record-store
   contract against it; it is green for the toy store and switched on per store by tickets 3,
   12, 13 and 14. Tests 1 to 4, 6 to 8, 23, 24 and 27 to 31 against the toy store. No real store
   uses it yet.
2. **The refused-records Notice and the doctor's lines** (Q3, R10, D1, C2). The one
   `records-unread` Notice with Update purlis and Set it aside and start fresh, a doctor line per
   refused store, and the doctor's counts of each store's skipped rows and set-aside copies. It
   lands before any real store can refuse. Test 26.
3. **Move the approval records**, rows 1 to 5, one or two per pull request: test 5 for the
   store first, then tests 12, 1 to 8 and 25 for it. The version is now read. Rows 4 and 5 gain
   "must not be tracked" (test 38). Row 1 moves through the extension registry's own calls (C7).
4. **The ledger interface and the four adapters**, with no change of behaviour: test 20 green,
   and every existing approval test still green.
5. **R3's lifecycle**: unpin forgets both, a rename carries both with R8's framing, a lone or
   unstamped Regenerate part reads New with B1's stamp, the two-file lock with `update_held` and
   the crash order, and approve replacing the entry whole. `docs/plane-format.md` gains the
   stamp first. Tests 13, 14, 16, 19, 35 and 37, and a changelog fragment: every Regenerate
   approval asks once.
6. **Removing a workspace forgets its pages' approvals** (Q11), from `core/wscmd/remove.rs`.
7. **R3's gate**: Regenerate refuses unless the page is Approved. Test 15 and a changelog
   fragment.
8. **R4, gap 17: the reach**, recorded on approval and checked at load and at every ask, with
   the plane grant bound to it, `docs/plane-format.md` gaining `reach` first, and the baseline
   shipped as runtime data (Q6). Tests 17, 18 and 39.
9. **Forget a harness declaration** (Q9): its removal through purlis, and Forget on its harness
   card.
10. **A moved extension carries its approval** under Q13's conditions (B2):
    `moved_clears_reading` changes. Test 36.
11. **R4, gap 14: the code-approval Notice**, split three ways (C3):
    - **11a. The Notice alone**: its family use, its story with keyboard, axe and screenshot, the
      core's sentences it carries (test 41). No adapter draws it yet.
    - **11b. One adapter per pull request**: the page and Regenerate (with Q5's one command that
      approves the listed parts under one lock, headed lines, and "Approve and run"), then the
      extension (test 22). Each removes its old re-ask and grows test 21's banned list.
    - **11c. The harness start flow, last**: the harness declaration and local profile onto the
      Notice, the Blocking ask for a held chat (R7), and the start carried through Approve and
      start. Tests 40 and 42, and a changelog fragment.
12. **The fifth adapter, a persona's MCP servers** (Q16, R11): row 6 onto the record store with
    "must not be tracked", the adapter answering Approved or New, and its Notice. Tests 38 and
    43 for it.
13. **Move the row stores**, one or a few per pull request: the dispatch queue with `refused/`
    (its `pf` entry first; tests 9, 32, 33 and 34), the dispatch record, the skill grades, the
    spool (test 11), and the other JSONL logs (tests 28 and 29). Test 5 first for each. Row 14's
    fail-closed answer is decided in its ticket.
14. **Move the remaining whole-file stores**, one or a few per pull request, test 5 first for
    each: `forge-accounts.json` (test 10), the machine store (its `.unread` copies no longer
    replaced and a newer file refused, with `pf:8791`-`:8793` changed first), the link hosts
    (row 7), the reopen record, and the rest of section 1.
15. **Remove `machine::write_beside`, `plane::write_private` and `glrefresh.rs:524`'s wrapper**
    once nothing calls them, move the forge's own writer (`forge/etag.rs:104`) onto
    `rewrite::replace`, and remove the doctor's copy of the `keys.json` check.
16. **The amendments** to ADRs 0122, 0069, 0112, 0053, 0073 and 0034, as files of their own.

Tickets 1 and 4 have no behaviour a person sees. Ticket 2 adds a Notice no one meets until a
store refuses. Ticket 3 is the first that could refuse a person's record, and only one written by
a newer purlis or one that is not a record. Tickets 5 to 12 are the ones a person notices.

## 5. What the order guarantees

- No store can refuse before the Notice that says so exists (ticket 2 before 3).
- No key is written before every reader of its store tolerates it (test 5 in the store's own
  ticket, before tickets 5 and 8 write a key).
- No old re-ask is removed before its adapter draws the Notice (11a before 11b and 11c).
- No adapter answers through the ledger before its record is on the store (ticket 3 before 4,
  ticket 12 alone for MCP).

## 6. Review round 1

Five reviewers read the ADR and this note at `0707537`, one axis each: A integrity, B security,
C buildability, D failure and recovery, E the person and the language. The operator's rulings
R6 to R11 settled the findings that needed a ruling; the rest were the steward's to fix. Each row
says where a finding was resolved, or why it was not changed. "ADR" is ADR 0125; "note" is this
document.

| Finding | Resolved |
| --- | --- |
| A1 | R7: ADR §7 (a held chat is a Blocking ask as well as the Notice), Decisions R7; note §2 harness row, test 40, ticket 11c |
| A2 | R6: ADR "Where purlis was" describes the machine store as it is; ADR §1 splits Damaged from Unreadable; ADR §2 damage rule; note row 22 |
| A3 | ADR §10 heading and bullets ("two added keys, one new folder"); note §1 new stores with tier |
| A4 | ADR §1 declares the root apart from the tier; note §1 declaration fields |
| A5 | ADR "Where purlis was" (locks) and §1 (per-store locks, `update_held`, no nesting); note test 30 |
| A6 | ADR §5 "What an approval of a page draws"; note §2 storage origin row |
| A7 | CONTEXT.md: each new entry names its concept (Project or Workspace) |
| A8 | CONTEXT.md: Reach renamed, Code approval lists the local profile and MCP servers with each tier, Private record names R9's exception, Regenerate no longer contradicts Q4 |
| A9 | ADR §5 and note §2 cite `approve_extension` and its call at `tauri/extensions.rs:1537`; the test code in `views.rs` is no longer cited |
| A10 | ADR "Where purlis was" and §3: `glrefresh.rs:524` wraps `plane::write_private`; only `forge/etag.rs:104` bypasses |
| A11 | ADR §1: a negative version is Unplaceable, each store names its version key, a refusal exits 1 (ADR 0120); note tests 2 and 3 |
| B1 | ADR §8 (the template stamp), §10; note §2, test 35, ticket 5 |
| B2 | ADR §5 extension row, Q13; note §2, test 36, ticket 10 |
| B3 | R8: ADR §8, Considered options; note §2, test 14 |
| B4 | ADR §1, §4 (approve replaces the entry whole); note test 37 |
| B5 | ADR §5 (a tracked record approves nothing); note declaration fields, test 38 |
| B6 | ADR §2 (`refused/` hardened); note new stores, test 34 |
| B7 | R9: ADR §2 (rebuild over is its own flag, never on `keys.json`); note row 49, test 31 |
| C1 | note ticket 1 merges the module, the harness and a toy store |
| C2 | note tickets 2 (Q3, doctor counts), 9 (Q9), 6 (Q11), 10 (Q13), 12 (Q16) |
| C3 | note ticket 11 split into 11a, 11b and 11c; note §2 harness row (`shown`, `asked.again`) |
| C4 | ADR §1 and note §1 name the fields. Not changed: the per-store serializer and shape are not tabulated for all 56 rows, because each is read and kept in the store's own ticket, which is sized as a rewrite |
| C5 | ADR §1 and §10 (`update_held`); note ticket 1 |
| C6 | R11: ADR §5 MCP row and decisions; note §2 MCP, test 43 |
| C7 | ADR §4 (the adapter wraps the registry); note §2 extension row, ticket 3 |
| C8 | ADR §6 (baseline as runtime data, extension query asks once); note test 5 per store, test 39 |
| C9 | note §1: rows 1 to 5 in ticket 3, row 6 in ticket 12, row 7 in ticket 14; ADR §10 counts what it adds |
| D1 | R6, R10: ADR §2 (Set it aside, fail-closed answer, one Notice); note fail-closed column, tests 24 to 26, ticket 2 |
| D2 | R6: ADR §2 damage rule with its own sentence; note row 22, test 23 |
| D3 | ADR §1 and §2 (TooOld and its migration); note test 27 |
| D4 | ADR §2 (a missing key is not a MAC failure); note rows 8 and 49, test 33 |
| D5 | ADR §2 (the floor never passes a skipped message); note test 32 |
| D6 | ADR §2 (pruning and the bound keep skipped rows); note row 15, test 29 |
| D7 | ADR §2 (a torn line is a skipped row); note test 28 |
| D8 | ADR §1 (per-store locks, busy sentence, old and new writers); note test 30. Not changed: an update still goes ahead unlocked where no lock can be taken, as `machine::Lock` does today, because refusing would lose every write rather than risk one |
| D9 | ADR §10 (crash order for approve, forget and moved, and why nothing retries a move); note test 19 |
| D10 | R9: ADR §2; note rows 12, 19, 20, 36, 47 and 49 |
| D11 | ADR §2 and §10; note new stores (bound, cleanup, tier) |
| D12 | ADR §6 (baseline kept for good, removed queries, extension queries, an older build's write); note test 39 |
| D13 | ADR §1 (a development build's version until a release ships it) |
| E1 | ADR §2: one sentence each for TooNew, TooOld, Unplaceable, damaged and unreadable, leading with what stopped working, the path and version in the detail |
| E2 | ADR §7 (`RUNS_AS_YOU`, `FINGERPRINTED`, the harness sentence); note test 41 |
| E3 | R7: the Blocking ask keeps a held chat reachable when the Notice is dismissed; ADR §7 says where each adapter's Notice is drawn. Not changed: a dismissed Notice is not kept in the Notices view, because Dismiss is the `Notice` family's and the ask already covers the held chat |
| E4 | ADR §4 and §6: "reach changed", `reach-changed`, "Now also reads" and "No longer reads"; CONTEXT.md |
| E5 | ADR §7 (headed lines, "Approve script and Regenerate code") |
| E6 | ADR §7 ("Approve and run", "Approve and start"); note test 42 |
| E7 | ADR §4 (`script-free`, `not-here`, `themes`, and `as-of` as no standing; Not asked against the gate) |
| E8 | CONTEXT.md, Code approval |
| E9 | CONTEXT.md: "Reach (of an artifact)"; ADR §7 and §9 no longer say "reach" for dispatch grants or link hosts; "harness-declaration adapter" in the ADR, this note and CONTEXT.md |
| E10 | ADR: "standing word" for "state word", "approval records" for "consent records", "drawing" gone, "ledger" said to be the code's word; CONTEXT.md defines **Subject** |
| E nit: one verb for the remedies | ADR §2: Update purlis; Set it aside and start fresh; Approve |
| E nit: unwrapped lines | ADR rewrapped |
| E nit: "grilled", "seams", "A-D" | ADR's opening explains each before use |
| E nit: Regenerate entry's lifecycle detail | CONTEXT.md, Regenerate |
| E nit: `record-refused` | the cause key is `records-unread`. Not changed: the queue's folder keeps the name `refused/`, which is a path Q15 named, not window copy |
