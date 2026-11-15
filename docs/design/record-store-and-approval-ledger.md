# The record store and the approval ledger: call sites, contract tests and tickets

This note is the working half of
[ADR 0125](../adr/0125-one-private-record-store-with-a-versioned-read-and-one-approval-ledger-for-code-that-runs.md),
for epic #6677. The ADR decides. This note lists what moves: every private record that would
move onto the record store, every approval adapter with its record, key and lifecycle callers,
the contract tests that must exist before anything moves, and a proposed order of tickets. It
proposes and does not decide. Where the ADR records a decision (Q1 to Q16, R6 to R11 from
review round 1, and R12 to R24 from review round 2), this note follows it and marks the row.
[Section 6](#6-review-round-1) says where each finding of review round 1 went, and
[section 7](#7-review-round-2) each finding of review round 2.

Every `path:line` was read at `2dc0d73c2`, except those added in review round 2, which were
read at `822ef516a`. Paths are shortened as in
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
ticket that moves the store pins it. A store marked **starting fresh grants more** (R13) is one
whose fresh, empty record would let through what the person refused: for it, Damaged is
refused like Unplaceable, so the fail-closed answer holds and writes refuse, and Set it aside
and start fresh is offered on the person's press, with a sentence naming what is forgotten.
Test 25 covers these stores too.

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
| Version key, own, oldest, migrations | `version`, `v`, or none (unstamped 1); TooOld's migration defaults to the damage rule (D3). A store that reads two versions today declares both (row 49 reads 1 and 2, `core/hookwire/spool.rs:995`) |
| Version history | for each version the store knows, the release that introduced it, or a development build's version until a release ships it (Q1, D13); it goes into `pf` and a TooOld refusal's detail (A-F15) |
| Bound | today's, moved into the declaration |
| Lock | blocking named lock (`core/machine.rs:2640`, `Lock::named`); `rewrite::Lock::bounded` (`core/rewrite.rs:209`); a named lock in the session host's folder waited on with a bound, which every approval record takes (rows 1 to 6; rows 2 and 3 share one, the page subject's; B-F6, D-F6, D-F7); the event log's own; the compare-and-swap of `skills.local.lock`; none today, one added. The lock file's name is recorded in `pf` and pinned by test 30, because it is a contract between builds (D-F6) |
| Busy sentence | the store's own; an approval's is "purlis is still recording another approval. Nothing was approved. Try again." |
| `{what}` | the store's name for itself, plural, in lower case ("your extension approvals"), because the sentences say "wrote them" and "were damaged" (E25) |
| What starting fresh costs | one sentence, drawn on the store's line before the press: for the approval records "every approval asks again", for the spool's keys "queued messages can no longer be checked, and the replay floors are gone" (E21, D-F1) |
| Starting fresh grants more | rows 33 (the taint file) and 40 (R13); each store's ticket checks its own record, among them the extension record's off set (row 1) and `runners-stopped.json` (row 35) |
| Serializer | `pyjson::dumps_indent2` (rows 2, 3), `pyjson::dumps_indent2_unicode` (row 6), sorted, serde pretty. Each ticket reads its store's and keeps it, so no file's bytes change |
| Keeps unknown keys | at which levels, or **refuses unknown keys**, for a store whose reader refuses one today (row 33's `pool-setup.json`, `core/poolsetup.rs`, `deny_unknown_fields`). About 45 of the stores do not keep them today, and each keeps today's answer unless its ticket says otherwise (A-F10, A-F15, C18) |
| Must not be tracked | rows 4, 5 and 6 (B5, R15): a tracked or un-ignored record approves nothing, and Approve refuses while it is so. The check uses `profiles`' hardened git runner (`core/profiles.rs:1588`, `git_ignore_state`; `:1614`, `kept_local_within`): Ignored passes, NotARepo passes, Tracked and Committable refuse, and Unknown (git missing, timed out, failed) approves nothing |
| Fail-closed answer | the column below |
| Dispatch MAC | row 8 checks its MAC over the exact bytes written, so its serializer is fixed by the MAC, not chosen |
| Registry | every declaration is in one static registry, with how a path that has a key in it (per chat, clone, account) is enumerated, so the window, the doctor and test 6 can list every store (C19). A test-only toy declaration is outside it |

**Approval records** (tickets 3a to 3c move rows 1 to 5; ticket 12 moves row 6; row 7 moves
with the whole-file stores in ticket 14d). Every approval record takes a named lock in the
session host's folder, which no chat opens, and waits on it with a bound, so its busy sentence
can be said and a chat cannot hold it (B-F6, D-F7). Rows 2 and 3 share one lock, the page
subject's, from the ticket that moves them (D-F6, A-F12):

| # | File | Module | Tier | Shape | Today | Under the store | Fail closed | Keeps unknown keys | Lock |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | `<config>/extensions.json` | `core/extension.rs:203` | Machine, device-bound (`pf:8817`) | whole | refused (`:2473`) | refuse | ✱ no extension stands approved, so none runs; approving refuses. Its ticket checks whether the off set of built-ins grants more when started fresh (R13) | no | none (`:2624`); named, bounded |
| 2 | `<config>/artifact-approvals.json` ⚠ | `core/artifact/approval.rs:35` | Machine, device-bound (`pf:8813`) | whole | unread (`:432`); a bad file reads as empty and the next approval writes only itself | refuse | ✱ no page's script runs; approving refuses | entries yes, top level no | none; the page subject's named lock, bounded |
| 3 | `<config>/artifact-regenerate-approvals.json` ⚠ | `core/artifact/regenerate.rs:49` | Machine, device-bound (`pf:8814`) | whole | unread (`:495`); as 2 | refuse | ✱ no Regenerate runs; approving refuses | entries yes, top level no | none; the page subject's named lock, bounded |
| 4 | `.charter/harness-declarations-approved.json` ⚠ | `core/harness_declaration.rs:54` | Clone state (`pf:1124`) | whole | none; a bad file reads `{}` and the next approval writes only itself | refuse | ✱ no declaration stands approved; a chat on one is held, as a Blocking ask (R7) | while it parses | none; named in the session host's folder, bounded |
| 5 | `.charter/harness-profiles-launched.json` ⚠ | `core/profiletrust.rs:26` | Clone state (`pf:1010`) | whole | as 4 | refuse | ✱ as 4, for local profiles | while it parses | none; named in the session host's folder, bounded |
| 6 | `.charter/mcp-approved.json` ⚠ | `core/personaverbs/mcp.rs:32` | Clone state (`pf:4806`, listed again at `pf:8158`) | whole | none; as 4 | refuse; the fourth adapter, `mcp` (Q16 counts it fifth, by records; R11) | every credentialed server is withheld from chats | while it parses | none; named in the session host's folder, bounded |
| 7 | `<config>/artifact-link-hosts.json` | `core/artifact/links.rs:32` | Machine, device-bound (`pf:8816`) | whole | unread | refuse | no host stands allowed, so each link asks | no | none |

**Rows and files of one record each** (tickets 13a to 13d2; row 18 goes with ticket 13c, row 9
with ticket 14e, row 58 with ticket 13d2). A row store never locks out, apart from rows 14 and
20, whose answers R18 and R19 decide: only the row it cannot place is left unread.

| # | File | Module | Tier | Today | Under the store | Fail closed | Lock |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 8 | `.charter/app/spool/messages/<chat>/<seq>.json` | `core/dispatchqueue.rs:49` | Machine, device-bound (`pf:7928`; no heading of its own) | **deleted** (`:625`; `hookwire/relay/carrying.rs:402`), and by every other remover: `drop_unchecked` (`core/dispatchqueue.rs:724`, also called at `carrying.rs:213`, `:222`), `forget` (`core/hookwire/spool.rs:1594`, `remove_dir_all` at `:1664`) through `forget_chat`, `forget_all_but` and `forget_ended_at_open`, and `undelivered::set_aside` (`core/dispatchqueue/undelivered.rs:153`), which reports only messages that check (D-F2) | skip, listed as waiting (ADR §2); the drain stops at the lowest skipped message, as it stops at a `.part` (`core/dispatchqueue.rs:583`), and later messages wait (R16); a bad MAC under a key that was read is moved aside into `refused/` (Q15); keys unavailable leave it waiting (D4); keys readable with no key for it report it undelivered and keep it (R17). An unparseable message is a skipped row; an oversize one is Unreadable | the message waits | none (made once) |
| 9 | `.charter/app/dispatches/<ULID>.json` | `core/dispatchrecord.rs:73` | Clone state (`pf:7392`) | skipped | skip | that dispatch is not listed; the doctor counts it | flock |
| 10 | `.charter/app/spool/<chat>/<key>-<seq>` | `core/hookwire/spool.rs:96` | Clone state, transient (`pf:7829`) | unread; an unparseable line is deleted | skip (gap 7) | the line waits, undelivered | none (made once) |
| 11 | `…/skill-grades/<YYYY-MM>.jsonl` and `routers/` | `core/skills/grades.rs:35`, `core/skills/grades/routers.rs:39` | Machine, device-bound (`pf:9029`, `:9038`) | skipped | skip | that grade is not counted | none (append) |
| 12 | `…/skill-grade-resume/<grade>.json` | `core/skills/grades/resumable.rs:40` | Machine, device-bound, transient (`pf:9152`) | skipped | skip (R9: transient is not rebuilt over) | that grade is not resumed | none |
| 13 | `<data>/events/<device>/events.jsonl` | `core/eventlog.rs:79` | Machine, device-bound (`pf:8841`) | `v` read, never checked | skip | that event is not read | its own flock |
| 14 | `<config>/kill-switch.jsonl` | `core/halt.rs:35` | Machine, device-bound (`pf:8804`) | none; a bad line is skipped (`core/halt.rs:139`, `stopped_by_journal_in`); 1000 lines kept (`:38`, `JOURNAL_LINES`) | skip | ✱ (R18) a last line this build cannot read counts as halted, and the window says why; this build's own re-arm written after it ends the halt; an Unreadable journal with the marker present counts as halted; a stop or a re-arm is never refused for the bound or the cap, and skipped rows are kept past the cap; one build's re-arm is never read as tampering by another | none |
| 15 | `<config home>/runner-hours.jsonl` | `core/runnerhours.rs:32` | Machine, device-bound (`pf:8874`) | none; a bad line is skipped and dropped on rewrite | skip; never pruned (`:258`, `pruned`) or dropped for the bound (D6) | that line's hours are not counted | flock |
| 16 | `<data>/network/<key>.jsonl` | `core/sandboxblock_record.rs:46` | Machine, device-bound (`pf:8842`) | none; skipped and kept | skip | that block is not listed | flock only with a config home |
| 17 | `.charter/save-journal.jsonl` | `core/planegit.rs:3383` | Clone state (`pf:8417`) | none; skipped and kept | skip | that entry is not replayed | none |
| 18 | `.charter/app/arrived/<chat>.json` | `core/sandbox/arrived.rs:407` | Clone state, rebuildable (`pf:7531`) | overwritten (stamped 1) | rebuild over (R9) | rebuilt | flock |
| 19 | `…/skill-levels/<chat>.json` | `core/skills/levels.rs:26` | Machine, device-bound, transient (`pf:9256`) | none; replaced | refuse (R9) | no level kept for that chat | none |
| 20 | `<config>/forge-budget/<account>.json` | `core/forge/budget.rs:236` | Machine, device-bound, transient (`pf:8819`) | none; overwritten | refuse (R9) | ✱ (R19) a budget this build cannot read counts as spent for background requests, never for a request a person waits on, as today's rule for a spent hour (`core/forge/budget.rs:316`, `admit`); listed in `records-unread` | flock; its lock name kept (`:445`, `lock_name`) |
| 21 | `<config>/forge-etags/…/<sha>.json` | `core/forge/etag.rs:69` | Machine, device-bound, rebuildable (`pf:8818`) | none; replaced | rebuild over (R9) | rebuilt | none |
| 58 | a LOCAL workspace's `workspaces/<ws>/work/<device>.jsonl` | `core/work/log.rs:208`, `append` | Clone state when LOCAL (`pf:2371`); the LIVE file is the Plane's and keeps its read | skipped and counted (`:805`); append only | skip | that link is not read | none (append) |

**Whole-file records** (tickets 14a to 14e6; row 7 in 14d, row 49 in 13b, row 35 in 14d4):

| # | File | Module | Tier | Today | Under the store | Fail closed | Keeps unknown keys | Lock |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 22 | `<config>/machine.json` | `core/machine.rs:171` | Machine, device-bound (`pf:8791`) | another version, or a file that does not parse, is copied to `.unread` and overwritten (`:2561`, `update`; `:2595`, `keep_unread`); a file it cannot read at all is never overwritten | refuse a newer one (R2), with Set it aside and start fresh; a damaged one is copied aside, never replacing an earlier copy, and starts fresh (R6). This build never writes the bare `.unread` name, which every older build replaces, and always writes `.unread-<UTC>` (D-F18) | ✱ no plane, approval of a plane or pool is known; a chat in a workspace with no place chosen is refused, as for an unreadable file today | no | blocking named lock (`:2640`) |
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
| 33 | `runners.json`, `runners-left.json`, `runners-unsandboxed.json`, `pool-setup.json`, `peers.json` | `core/runners.rs:47`, `runners/left.rs:29`, `runnertaint.rs:25`, `poolsetup.rs:49`, `runnerlink.rs:38` | Machine, device-bound (`pf:8873`, `:8878`, `:8877`, `:8879`, `:8880`) | refused | refuse | no pool or runner is known, so no chat starts on one. For `runners-unsandboxed.json`, every runner counts as tainted, and the store is marked starting fresh grants more (R13, B-F1; `core/runnertaint.rs:17`) | `pool-setup.json` refuses an unknown key today and keeps refusing; each other file's test 5 records its answer (A-F10, C18) | flock |
| 34 | `<config home>/charterd/window-names.json` | `core/windownames.rs:26` | Machine, device-bound (`pf:8808`) | overwritten (forced to 1) | refuse | windows keep their default names | no | flock |
| 35 | `<config home>/purlisd/runners-stopped.json` | `crates/host/src/reach.rs:53`, `RUNNERS_STOPPED` | Machine, device-bound (`pf:8810`) | none; overwritten | **the one exception (R23):** it stays out of the record store, because `crates/host` does not link `purlis-core` (`crates/host/Cargo.toml:14`). Its versioned read is fixed in place in the host: it refuses a newer version and never overwrites. Its ticket checks whether starting fresh grants more (R13) | no runner is known as stopped | no | mutex |
| 36 | `<data>/inbox/<key>.json` and `.snoozed`, `.baseline`, `.seen`, `rules.json` | `core/inboxupdates.rs:34`, `askrules.rs:37` | Machine, device-bound (`pf:8846`-`:8850`); the first is transient | none; overwritten | refuse (R9: the transient one too) | no update, snooze or rule is known | no | flock |
| 37 | `.charter/app/spool/messages/records.json` | `core/dispatchtalk.rs:304` | Machine, device-bound (`pf:7928`) | overwritten (stamped 2) | refuse | no record of a message is shown | no | none |
| 38 | `.charter/app/dispatches/ended.json` | `core/dispatchrecord/ended.rs:38` | Clone state, rebuildable (`pf:7420`) | overwritten | rebuild over (R9) | rebuilt | no | none |
| 39 | `.charter/app/dispatches/refused-while-away.json` | `core/dispatchaway.rs:86` | Clone state (`pf:7511`) | refused | refuse | nothing is shown as blocked while away | no | flock |
| 40 | `.charter/app/dispatch-never.json` | `core/dispatchnever.rs:60` | Clone state (`pf:6664`) | none; a bad entry refuses the change (`core/dispatchnever.rs:9`) | refuse; marked starting fresh grants more, so Damaged is refused too (R13, B-F1) | every pair counts as never, so no dispatch goes | yes | flock |
| 41 | `.charter/app/chat-records/closed.json` ⚠ | `core/chatrecords.rs:50` | Clone state (`pf:7610`) | overwritten, earlier entries lost | refuse | no closed chat is listed | partly | flock |
| 42 | `.charter/app/reopen.json` | `core/reopen.rs:49` | Clone state (`pf:6847`) | overwritten | refuse | nothing reopens | no | none |
| 43 | `<data>/app-state/<clone>/studio-chats.json` ⚠ | `core/reopen/studio_chats.rs:28` | Machine, device-bound (`pf:9012`) | overwritten, earlier entries lost | refuse | no studio chat reopens | partly | flock |
| 44 | `.charter/app/secret-asks.json` | `core/secrets/ask.rs:79` | Clone state (`pf:6818`) | overwritten (stamped 1) | refuse | ✱ no secret ask is answered or made | no | mutex |
| 45 | `<config home>/purlis/sandbox-grants/<clone>.json` and `every-project.json` | `core/sandbox/local.rs:41`, `:1453` | Machine, device-bound (`pf:4738`) | none; overwritten | refuse | no grant of this machine counts | no | flock |
| 46 | `.charter/app/browsers-at-quit.json` | `core/browsersatquit.rs:26` | Clone state, rebuildable (`pf:6678`) | none; deleted on read | rebuild over (R9) | no browser comes back | no | none |
| 47 | `.charter/app/browser-timelines/<chat>/timeline.json` | `core/browsertimeline.rs:48` | Clone state, transient (`pf:6713`) | none; a bad list deletes the chat's folder | refuse (R9) | that chat's timeline is not shown or added to | no | mutex |
| 48 | `.charter/app/mirrors-offered.json` | `core/guest.rs:480` | Clone state (`pf:6746`) | none; overwritten | refuse | no mirror is offered | no | flock |
| 49 | `.charter/app/spool/keys.json` | `core/hookwire/spool.rs:101` | Clone state, transient (`pf:8014`) | refused (and the doctor checks it again, `core/doctor/spool.rs:50`); reads versions 1 and 2 (`:101`, `:104`) | refuse (R9: never rebuilt over, it holds the replay floors); the doctor asks the store; declared with oldest 1 and own 2 and its migration. Once it is started fresh, a queued message whose key it no longer holds is reported undelivered and kept, never moved to `refused/` (R17, D-F1, B-F16) | ✱ no message is delivered, and none is moved to `refused/` (D4) | no | flock |
| 50 | `app/spool/messages/waits.json` (runner) ⚠ | `core/hookwire/relay/carrying.rs:88` | Machine, device-bound (`pf:7951`) | overwritten, queued messages retired | refuse | no message is retired; they wait | no | mutex |
| 51 | `…/skill-grade-after-edit.json`, `…/skill-earlier-names.json` | `core/skills/grades/after_edit.rs:33`, `core/skills/grades/earlier.rs:45` | Machine, device-bound (`pf:9070`, `:9098`) | overwritten | refuse | not kept | no | flock |
| 52 | `skills.local.lock` | `core/skills/source/lock.rs:26` | Clone state (`pf:1767`) | refused | refuse | no local skill source is changed | no | compare-and-swap |
| 53 | `.charter/cache/skill-sources-checked.json` | `core/skills/source/checked.rs:31` | Clone state, rebuildable (`pf:8663`) | overwritten | rebuild over (R9) | rebuilt | no | mutex |
| 54 | `.charter/plane-push.json`, `.charter/save-branch.json` | `core/planegit.rs:277`, `planegit/prsave.rs:280` | Clone state (`pf:8184`, `:8226`) | none | refuse | no automatic push or save branch | no | none |
| 55 | `.charter/guard-seen.json` | `core/guardseen.rs:30` | Clone state, rebuildable, legacy (`pf:8136`) | none; rebuilt | rebuild over (R9) | rebuilt | no | none |
| 56 | `<config home>/signing/pushed.json`, `commit-emails.json`; `.charter/signing/waiting.json` | `core/signingsetup.rs:520`, `:620`; `signingsetup/retry.rs:109` | **not in `docs/plane-format.md`**: add them first | none; overwritten | refuse | signing setup is not retried | no | none |
| 57 | `.charter/cache/glstate.json` | `core/cistate.rs:45`, `CACHE`; written by `core/glrefresh.rs:475`, `save`, through the wrapper ticket 15 removes | Clone state, rebuildable (`pf:8579`) | none; replaced | rebuild over (R9) | rebuilt; the CI column is blank until then | no | none |

**New stores this effort adds**, each with its `pf` entry and tier before the code writes it
(A3, D11):

| File | Tier | Holds | Bound and cleanup |
| --- | --- | --- | --- |
| `.charter/app/spool/messages/refused/` | Machine, device-bound, as row 8 | dispatch messages whose MAC did not match under a key that was read (Q15), `0700`, denied to every chat's sandbox. Before any move it is checked without following a link: a real folder, owned by this user, mode `0700`; otherwise nothing is moved and the doctor says so. A moved file is named `<chat>-<seq>.json` (B-F15) | at most 64; past that a failing message stays where it is, listed; nothing removes one but the person |
| `<file>.unread`, `<file>.unread-<UTC time>`, `<file>.set-aside-<UTC time>` beside any whole-file store (together, its **kept copies**) | its store's | a damaged record **copied aside** (R6), or a record **set aside** on the person's press. Each is made by an exclusive, no-follow `0600` create through the gated walk from the store's root, with a counter suffix when the name is taken (R14). Beside a record the sandbox holds (rows 4, 5, 6), every sibling whose name starts with the record's name is denied to a chat, under every spelling of the state folder and under `$PURLIS_HOME` (R14, amending ADR 0067) | never replaced; the doctor counts and names them, and says how to put one back; nothing removes one but the person |

**Out of scope:** the Plane tier (`charter.toml`, `pins.toml`, `skills.lock`, workspace markdown,
`personas/_*/*.jsonl`, and a LIVE workspace's `work/<device>.jsonl`, whose LOCAL copy is row
58), plain-text markers and pointer files, `runners-stopped.json` (row 35, R23, fixed in place),
`.charter/cache/repostate.json` and `.charter/cache/harness-wiring.json` (`pf:8569`, `pf:5902`),
which no Rust code reads or writes (they are written by the legacy Python `charter` only), and
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
- **Row 35 is the one exception** (R23). The host crate does not link the core, so its record
  keeps a hand-written versioned read in `crates/host`, which refuses a newer version and never
  overwrites.
- `docs/plane-format.md` is missing rows 28 and 56, has no heading for row 8, and lists row 6
  twice. Each is fixed in the ticket that moves the store. Row 22's `.unread` entry
  (`pf:8793`) changes from "one copy, the latest" to copies never replaced.

## 2. The approval adapters

The ledger has four adapters, and the cause key's adapter word is theirs: `extension`, `page`
(one subject of two parts, the script and the Regenerate code, from ticket 4 on), `harness` (the
declaration with its local profile) and `mcp`. They keep six records. Q16 calls MCP the fifth
because it counts the four subjects R3 names with the page and its Regenerate apart (A-F14,
C22, E28).

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
| Re-ask today | `ui/ApproveExtension.tsx:59` (a `Dialog`, "Trust it anyway" at `:73`), which is the first ask too ("Trust it"); the row in `ui/Extensions.tsx` |
| Moves to | the code-approval Notice, carrying `RUNS_AS_YOU` and `FINGERPRINTED` (`core/extension.rs:321`, `:339`) as given (E2). The whole `ApproveExtension` dialog goes, and its "What it declares" list becomes the Notice's "what it may do" (E15). The Extensions row keeps its standing word, which is a press that opens the Notice again (R20) |

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
| Gated at use | page: every load and every ask on the port (`tauri/artifacts.rs:447`, `:654`, `:1012`; `tauri/artifacts/calls.rs:88`, `:242`); Regenerate: `tauri/regenerate.rs:232`, `run_pressed` (its standing at `:243`), and `:118`, `artifact_regenerate_about` (`:146`). The new page gate is one core function, page standing and Regenerate standing in, run or refusal out, that both call, and `regenerate --check` too (C11). It passes only Approved and `script-free` (B-F9), and the declaration it runs, the fingerprint it checks and the stamp it compares come from one read of the template |
| CLI | `crates/purlis-cli/src/artifact.rs:438` (`regenerate --check`); `purlis artifact approve-status` (`core/artifact/tools.rs:185`) |
| Re-ask today | page: `ui/ArtifactTab.tsx:236`, `ScriptStanding`, and `:264`, `standingSaid`, whose six sentences hold three re-asks (`new`, `changed`, `pin-changed`) and two that are not asks (`not-here`, `as-of`); Regenerate: `ui/ArtifactRegenerate.tsx:144` (`AlertDialog`, "Approve and run" at `:157`) and `:165`, `asksBecause` |
| Moves to | one code-approval Notice per subject, in the artifact tab's band and in Notices, with a headed line per part and "Approve script and Regenerate code" when both ask (E5). It is raised when the page is loaded or its Regenerate pressed, never by a scan of every pin (E27). The page bar keeps a standing word per standing, which is a press that opens the Notice again (R20), and keeps the `not-here` and `as-of` sentences, which never ask (E12) |

### Harness declaration, with the local profile

| | |
| --- | --- |
| Records | `.charter/harness-declarations-approved.json` (`pf:1124`; `core/harness_declaration.rs:54`, `APPROVED`) and `.charter/harness-profiles-launched.json` (`pf:1010`; `core/profiletrust.rs:26`, `RECORD`). Both Clone state, and both stay there (ADR 0069, R3). Both declared "must not be tracked" (B5) |
| Key | the declaration's name; the profile's name |
| Fingerprint | the declaration's whole digest; the profile's kind, command and environment as declared (`core/profiletrust.rs:262`, `fingerprint`) |
| Standing today | `core/profiletrust.rs:244`, `Approval` (`New`, `Changed`); approved is the absence of a value |
| Read | `core/profiletrust.rs:133`, `read_record`: no version field, and a malformed or missing file reads as empty |
| Write | `core/profiletrust.rs:167`, `record`: keeps other entries; no lock |
| Approved | `tauri/lib.rs:1865`, `approve_profile`, which takes the `shown` string, not a fingerprint, and calls `core/profiletrust.rs:351`, `approve`. The start dialog then starts the same chat again (`ui/StartApproval.tsx:105`, `asked.again`), so the Notice and the ask must carry that start (C3). Approve refuses while the record is tracked or not ignored (R15) |
| Forgotten | nothing today. R22 (narrowing Q9): a Forget press on the harness card (`ui/HarnessCard.tsx:28`, `HarnessSummary`, which has no action today), labelled "Forget approval" (ticket 9). Removal through purlis waits until a command that removes a declaration exists: `purlis harness` has only list and show |
| Moved | not applicable: the record is inside the clone |
| Gated at use | `core/profiletrust.rs:312`, `declaration_approval_needed_in`, from every launch (`core/personaprofile.rs:156`) |
| Re-ask today | `ui/StartApproval.tsx:78`, `StartApproveAsk` (a dialog titled "Approve <profile>?", "Approve and start"), drawn at `ui/PlaneView.tsx:13322`; `StartApproval` is also used by `ui/SkillChatApproval.tsx` and `ui/artifactDiscuss.tsx`, so three start flows change (C13) |
| Moves to | the code-approval Notice, and a Blocking ask in Asks while a chat is held on it, both from the one standing (R7). The ask has Not now as well as Approve (R21). A chat is **held** from a start the window made that was refused for this subject until it is approved, answered Not now, cancelled, or the project closes (R21) |

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
| Moves to | the record store (row 6); the `mcp` adapter, after the other three (ticket 12); its Notice offers Copy command (ADR N2) and is added to `ui/Notice.guard.test.ts`'s `COPY_ONLY` list (`:898`), with its reason (C14) |

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

**Who owns a test.** Each test is owned by exactly one ticket, the one that writes it, named in
brackets after it. The harness tests (1 to 8, 23 to 25, 27 to 31 and 64) are written once
against a toy store and then **switched on** for each real store by the ticket that moves it,
which runs them and owns none of them. Test 5's pattern is written by ticket 3a, the first
ticket with a real store. New tests from review round 2 are numbered from 44.

**The record store, for every declared store** (run against a toy store in tickets 1 and 1b,
and switched on per store as it moves):

1. *A whole-file record of a newer version is never overwritten.* Write a record at
   `VERSION + 1`, run every write path of that store, and assert the bytes are unchanged and the
   write refused. The row case is test 64. [1]
2. *A newer whole-file record is refused, saying what stopped working.* The sentence leads with
   the store's name for itself and says a newer purlis wrote it; the detail names the path and
   the version found (ADR §2, E1). The error kind is pinned here; the CLI's exit code is test
   56, the first ticket with a command (A-F9). [1]
3. *A garbage version is not a missing one* (gap 2): `"8"`, `7.5`, `0` and `-1` are
   `Unplaceable`, with their own sentence, and a negative integer is never Understood (A11). [1]
4. *An unknown key survives a write*, at the top level and in an entry the write does not
   replace. [1]
5. *Today's reader tolerates an unknown key.* Run the store's reader as it is on main against a
   record carrying an extra key, and assert it reads the same as without it. It runs in the
   ticket that moves each store, before that store's first new key (C8). "First" means the
   first commit adds the test against the old reader, green; a later commit moves the store and
   the same test runs against the store's `read`. The pull request names the commit (C17). For
   a store whose reader refuses an unknown key today (`pool-setup.json`), the test pins that
   refusal instead, and the store is declared to refuse unknown keys (A-F10, C18). For rows 2
   and 3, ticket 3a also records the pre-effort reader's answer as a fixture, and tickets 5b
   and 8 switch the test on again with the real stamp and the real `reach`, asserting that every
   other entry still reads and that the old writer keeps them (D-F17). [3a]
6. *Every store the record store declares is in `docs/plane-format.md`* with its tier, beside
   the existing `every_store_the_plane_format_names_has_a_tier.rs`. It walks the registry. The
   toy store is test-only and outside the registry, and the test is seen red with a registered
   declaration that names a path the document lacks (C20, A nit N6). [1]
7. *Two writers do not lose each other's change*: two threads, then two processes, each
   updating a different entry, both entries kept. [1]
8. *A link, an oversize file and a non-file are Unreadable*, never Absent, and never written
   through. [1]
23. *A damaged whole file is copied aside, then the store starts fresh* (R6, D2): bytes that are
    not JSON read as `Damaged`; the next update leaves them byte for byte in `<file>.unread`;
    a second damage writes `<file>.unread-<time>` and leaves the first. With a live link, a
    dangling link, a folder and a plain file planted at `<file>.unread` and at the timed name
    first, nothing outside the folder changes and no planted file is replaced (R14). [1]
24. *Set it aside and start fresh* (R6, D1): for TooNew and Unplaceable, the press renames the
    file with no link followed and nothing replaced, the store reads `Absent`, and nothing is
    deleted. It is asked for by the store's id, never a path; under the store's lock it reads
    again and refuses unless the answer is still TooNew or Unplaceable (or Damaged, for a store
    where starting fresh grants more); the rename is a no-replace rename, and a target planted
    first gets a counter suffix (B-F13). [1]
25. *Each lockout store fails closed* (D1): every ✱ row, refused, does only its declared answer,
    and approving or changing refuses with the store's sentence. It covers too every store
    marked starting fresh grants more (R13): Damaged is refused, writes refuse, and Set it aside
    on a press names what is forgotten. [1]
26. *One Notice for every refused store* (R10): three stores refused at once raise one
    `records-unread` Notice listing all three, and one Notice in all, since the window's doctor
    line leaves them to it (E23); it settles when the last reads again, and a line leaves when
    its store reads (E32). The cause key is keyed by the set of refused stores, so a store that
    refuses after a dismissal brings it back (R20). A path with control characters is shown on
    one line (B-F18). [2]
27. *TooOld runs the declared migration* (D3): a toy store whose oldest is 2 meets version 1 and
    applies its migration, by default the damage rule. [1]
28. *A torn last line is a skipped row* (D7), kept on disk, and the next append starts on a
    fresh line. [1b]
29. *Pruning and the bound keep skipped rows* (D6): a toy row store with a newer row past its
    keep window keeps it through pruning; skipped rows alone over the bound refuse an append with
    the store's sentence. Switched on, it is `runner-hours.jsonl` past `KEPT_FOR` through
    `pruned`. A store with a line cap counts only this build's rows against it (D-F20). [1b]
30. *Locks* (D8): a lock file an older purlis takes (`machine.json.lock`) excludes the store's
    update; a bounded lock held past its wait refuses with the store's busy sentence, never
    `rewrite`'s chat sentence for an approval; two stores never wait on each other, except the
    two halves of the page subject, which share one lock; where `flock` is unsupported the update
    goes ahead. The declared lock file's name is the one `pf` records (D-F6). [1]
31. *Only rebuildable stores are rebuilt over a newer one* (R9, D10, B7): a transient store
    (the forge budget) and the spool's `keys.json` at a newer version are refused; a rebuildable
    one (`forge-etags`) is rebuilt. Rebuild over is a flag on any shape, so it is tested with the
    whole-file store. [1]
57. *The store's own failure paths* (D-F8): the copy aside fails (a full disk, a read-only
    folder), so the update refuses and the damaged bytes stay; two copies in the same second get
    a counter suffix; an update whose write is over the bound refuses with the store's sentence;
    Set it aside racing an update re-reads under the lock. [1]
64. *A newer row is never overwritten*: a row store with a row at `VERSION + 1` keeps it, in its
    place, through every rewrite; a file-per-record store leaves the file. [1b]
58. *Damaged and TooOld are said* (E14, D-F9): a damaged toy store raises its line in
    `records-unread` from the read, before any update, and the line names the copy once it
    exists; a TooOld store's line follows its migration; each has Dismiss. [2]
59. *Set it aside from the person's terminal* (D-F10): a command's refusal of a TooNew store names
    a remedy besides Update purlis; the terminal route sets a store aside by its name, with the
    same code as the window; it refuses when run from a chat, as `purlis persona approve-mcp` does
    (`core/personaverbs/approve.rs:122`); no purlis MCP tool sets a record aside. [2b]

**Pinning today's answers that the ADR changes** (red first, then green once the store moves):

9. The dispatch queue: a message of `v: 2` stays on disk and is listed as waiting
   (`core/dispatchqueue.rs`; today removed, surfaces section 12, UNTESTED). It is still there
   and still listed after every remover runs: the chat ends and the project is opened again
   (`forget_chat`, `forget_all_but`, `forget_ended_at_open`), the host's retire pass runs
   (`drop_unchecked`), and `undelivered::set_aside` reports the chat's queue (D-F2). [13a]
10. `forge-accounts.json` at version 2 is refused and kept (`core/forge/accounts.rs`; today
    stamped back to 1, UNTESTED). [14a]
11. Spool `v` other than 1 and `keys.json` at version 3 (`core/hookwire/spool.rs`; UNTESTED).
    [13b]
12. The page and Regenerate records at version 2 are refused, nothing stands Approved, and
    approving refuses (today the version is never read). [3a]
32. *The drain stops at a skipped message* (D5, R16): a `v: 2` message at seq 4 is kept; seq 5
    waits behind it and is listed, and the floor never rises past 3; a newer purlis delivers 4,
    then 5; the person removing seq 4, whose path is listed, lets 5 through. [13a]
33. *A key that is unavailable leaves messages waiting* (D4), *and a missing key is not
    tampering* (R17): with `keys.json` refused, damaged or unreadable, no message moves to
    `refused/`; with `keys.json` readable and holding no key for a message, the message is
    reported undelivered, as an ended chat's queue is, kept on disk and never moved to
    `refused/`; only a key that was read and a MAC that still fails moves one. [13a1]
34. *`refused/` is hardened* (B6, D11, B-F15): it is made `0700`; with a link, a `0777` folder or
    a file planted at `refused` first, nothing is moved and the doctor says so; a message is
    moved as `<chat>-<seq>.json` with no link followed and nothing replaced; the 65th failing
    message stays where it was; the listing shows the count, seqs and paths and no body;
    `chats_in` never lists it; the sandbox rule names `refused/` for both spellings of the state
    folder, so the assertion can fail. [13a1]
48. *The kill switch fails closed* (R18): a last line this build cannot read counts as halted,
    and the window says why; this build's re-arm written after it ends the halt; an Unreadable
    journal with the marker present counts as halted, and with the marker absent does not; a stop
    and a re-arm each succeed with skipped rows filling the bound and the 1000-line cap, which
    keeps them; with two builds running, one build's re-arm is not read as tampering by the
    other. [13d1]
49. *A spool `keys.json` started fresh moves nothing to `refused/`* (R17, D-F1, B-F16): damage
    `keys.json` and run one update so the damage rule starts it fresh, then set a refused one
    aside; each time, queued messages are reported undelivered and kept, and none is moved. [13b]
55. *The host's stop record refuses a newer one* (R23): `runners-stopped.json` at a newer
    version is refused, kept byte for byte and never written over, and the host's answer is
    that no runner is known as stopped. [14d4]
56. *A command that meets a refused record exits 1* (ADR 0120, A-F9): `purlis artifact
    approve-status` over a version-2 approval record exits 1 with the same sentence the window
    and the doctor say. [3a]
63. *A copy of a delivered message is never delivered twice* (R16, B-F14): with seq 4 skipped,
    a delivered message's bytes put back in the queue are not delivered again. [13a]

**The ledger:**

13. *Unpinning forgets both parts* (gap 19's UNTESTED Regenerate unpin case). Approve the page
    and its Regenerate, unpin, pin again with the same id, and assert both ask. [5]
14. *A rename carries both parts* (gap 19's rename case), and the Regenerate fingerprint is not
    computed again (R8). Approve both, rename the workspace, and assert both stand Approved at
    the new path. A Regenerate approval in the old framing asks once. [5b]
15. *Regenerate refuses for a page that is not Approved* (gap 16): the gate's core function
    refuses the run for every standing the type has except Approved and `script-free`, so a
    `New`, `Changed`, `PinChanged` and `NotHere` page each refuse it, and a standing added later
    (ticket 8's `reach-changed`) is refused with no new case (B-F9). `run_pressed`,
    `artifact_regenerate_about` and `regenerate --check` call that one function (C11); a
    script-free page passes (Q4). [7]
16. *A Regenerate part with no page part reads as New* (ADR §10). [5]
17. *A changed reach asks again* (gap 17): approve a page declaring `purlis.todo_list@1`, change
    the catalogue in a test build so the query is `plane`-scoped or gains a widening parameter,
    and assert `Changed(reach)`, a script-free render, and `not-approved` on the port. Then the
    same with an extension whose manifest widens its query, and with one that narrows it. Each
    time the standing carries the two lists, "Now also reads" and "No longer reads", compared
    line by line with the lines the entry holds, and only the changed line is in them (R12). [8]
18. *The plane grant does not survive a reach change*: `plane_granted` is false after the reach
    changes and before any re-approval, for an entry with a `reach` and for one without, whose
    grant holds exactly while it passes the baseline; the grant press refuses unless the reach
    it was shown is the reach now (B-F11, D-F16). [8]
19. *Every crash between two writes fails closed* (D9): fail the second write of approve, forget
    and moved in turn, and assert Regenerate is refused each time and the page asks. The approve
    case runs the ledger's two-part approve, and the gate (ticket 7) refuses the run (A-F3,
    D-F12). [5]
20. *Every adapter answers the same four events*: a table-driven test over the adapters, each
    declaring what it does for approve, change, forget and move, so a new adapter cannot leave
    one out. [4]
21. *No old re-ask comes back*: a guard test, beside `ui/Notice.guard.test.ts`, seen red with a
    fixture that fails it (C25). Each adapter's ticket adds its names to the banned list:
    `ScriptStanding`'s three re-ask sentences and the Approve script button (11b1), the
    Regenerate `AlertDialog` and `asksBecause` (11b1b), `ApproveExtension` (11b2), and
    `StartApproveAsk` (11c). Each adapter's rendering is its own test: 52, 47, 53, 40 and 62. [11a]
22. *A re-install of a changed extension shows the re-ask*, not the first ask (gap 14). [11b2]
35. *The template stamp is compared* (B1, B-F3), in three cases, each seen red with the stamp
    comparison removed: approve a page and its Regenerate, then change the template in place to a
    script-free one, with no unpin, and Regenerate stands New and its run is refused; an orphan
    left by an older build's unpin (a Regenerate entry, no page entry, no stamp) under a
    script-free re-pin stands New; a stamped entry whose stamp is another template's stands New.
    [5b]
36. *A moved extension carries its approval only under Q13's conditions* (B2): a clean copy
    carries; a copy with a file in its state folder, one with a relative link resolving outside
    the folder, and one with a link resolving into its state folder each ask. Only the window's
    install press carries, and a guard lists `extension::install`'s callers; the prompt-reading
    switch is carried only when the approval is (B-F17). [10]
37. *Approve replaces the subject's entry whole* (B4): an unknown key and the `plane` flag on the
    approved entry are gone after approve (B-F19); one on another entry, at the top level, and on
    an entry `moved` carries is kept. [5]
38. *A tracked Clone-state approval record approves nothing* (B5, R15): rows 4 and 5, committed or
    not ignored, read as nothing approved; Approve refuses while it is so, with the sentence and
    its cause (tracked, or not ignored) and path in the detail; git missing, timed out or failing
    approves nothing; outside a repo the record passes; the sentence settles once the file is
    ignored and untracked. Ticket 12 switches it on for row 6. [3c]
39. *A page with no `reach`* (D12, C8, D-F5): compared with the baseline, it stands Approved while
    nothing it declares reads more; a test catalogue that moves a declared core query to `plane`,
    or gives it a widening parameter, makes it stand `Changed(reach)`; a query absent from the
    baseline asks; a declared extension query asks once; an approval by an older purlis, which
    writes no `reach`, falls back to the baseline. The baseline is a checked-in file compiled into
    the binary, not generated from the catalogue, and the test pins its SHA-256, with a comment
    that a change needs a ruling (B-F12). [8]
40. *A held chat is one Blocking ask and the Notice* (R7, E3, R21): a harness declaration changed
    while two chats wait on it raises one ask in Asks and one Notice; an answer in either
    settles both; dismissing the Notice leaves the ask; each chat's refusal names the row; Not now
    puts the ask away while the project is open and ends the hold; cancelling a chat, or closing
    the project, ends its hold; the settle guard holds Approve (ADR 0097). [11c]
41. *The Notice says how code runs* (E2, E19): the core's how-it-runs sentence for each adapter
    renders as given: an extension's `RUNS_AS_YOU` and `FINGERPRINTED`, a harness's, a page
    script's, Regenerate's (today's words, `ui/ArtifactRegenerate.tsx:151`) and an MCP server's.
    A name with control characters renders on one line (B-F18). [11a]
42. *"Approve and run" finishes the press* (E6, C3): it approves then runs Regenerate once, through
    `run_pressed`'s own check. It is offered only when opened by a Regenerate press on a page that
    stands Approved (E36). The start half is test 65. [11b1b]
43. *The MCP adapter answers only Approved or New* (R11), and its approve replaces the persona's
    set and keeps every other persona's. [12]
44. *Only the person approves* (B-F7, C23, E30): a guard test lists every caller of the ledger's
    `approve` (the window's approve commands and `core/personaverbs/approve.rs:110`) and fails on
    any other; no CLI verb, hook or purlis MCP tool approves a page, an extension or a harness. [4]
45. *A failed forget refuses the unpin* (D-F15): unpin forgets first and removes the pin second; a
    forget that fails (a busy lock, a refused record) leaves the pin and its approvals as they
    were; a crash after the forget leaves the pin, and its page asks. [5]
46. *Removing a workspace forgets its pages* (Q11, D-F15): approve a page and its Regenerate,
    remove the workspace, re-make it at the same path with the same pin, and both ask; with a
    failure injected between the forget and the removal, the workspace is still there and its
    pages ask. [6]
47. *The joint press approves exactly what it showed* (Q5, B-F7): a Notice listing only "Script"
    leaves the Regenerate part unapproved; a part whose fingerprint or reach moved since it was
    shown refuses the whole press, so nothing is approved and nothing runs. [11b1b]
50. *The sandbox denies a held record's siblings* (R14):
    `a_chat_never_writes_charters_integrity_state` (`core/sandbox/tests.rs:324`) covers
    `<record>.unread`, `<record>.unread-<UTC>`, `<record>.set-aside-<UTC>` and `<record>.lock`
    beside rows 4, 5 and 6, under every spelling of the state folder and under `$PURLIS_HOME`.
    [3c0]
51. *A dismissed code-approval Notice comes back when what it shows changes* (R20): its cause key
    carries a digest of the fingerprints and the reach it shows; the family is in `KEPT`
    (`ui/keptCauses.ts:19`) with its core source; a dismissed Notice stays hidden while the digest
    holds and comes back when it changes. [11a]
52. *A page asks through the Notice, and its bar keeps its words* (E12, E27, R20): a page's New
    and Changed (code, pin, reach) render the Notice with cause key `code-approval:page:<key>`;
    a widened query appears under "Now also reads" and no other line does (B-F4); dismissed, the
    bar's standing word opens it again; a `not-here` page and an `as-of` version each keep their
    sentence and raise no Notice; the Notice is raised on load, never by a scan of every pin;
    approving a page that has an origin says it starts with empty storage (E20). [11b1]
53. *An extension asks through the Notice* (E15, R20): an extension's New and Changed render the
    Notice with cause key `code-approval:extension:<id>`; dismissed, the Extensions row's
    standing word opens it again; `ApproveExtension` is gone. [11b2]
54. *Forgetting a harness declaration* (R22, B-F6): Forget approval on the harness card forgets
    the declaration's and its local profile's approvals, and the event reaches every window; a
    pull that removes the declaration keeps the record, so the same bytes added back stand
    approved; Forget succeeds while a chat holds a lock on `.charter/`. [9]
60. *A Blocking ask of no chat* (C13): an ask of a subject is counted once on the ✋, listed once
    in Asks, and notified once, however many chats wait; an answer from either place settles it;
    Not now keeps it away while the project is open; it is never offered a rule, and never
    answered by a rule, Quiet, a timeout or a bulk triage action; only a connection that holds
    the approval scope answers it, never one from a chat's process tree (B-F8). [11c0]
61. *The harness ask runs only what it showed* (B-F8): the Notice and the row show the profile's
    whole line (`profiletrust::shown`); Approve is refused when what would be shown now differs;
    "Approve and start" approves, then starts through the full launch gate, and a declaration
    changed in between starts nothing and asks again; a refused dispatch raises no notification;
    each of the three start flows (the plane's picker, a skill chat, an artifact discussion)
    opens Asks on the subject's row. [11c]
62. *The MCP adapter's Notice* (A-F11, C14): an unapproved credentialed server raises the Notice
    with cause key `code-approval:mcp:<persona>`, its how-it-runs line and Copy command for
    `purlis persona approve-mcp`; `COPY_ONLY` lists it with its reason. [12]
65. *"Approve and start" finishes the press* (E6, C3): it approves, then starts the held chat
    again, once; with two chats held, its label names the count and both start. [11c]

## 4. Order of tickets

Each ticket is one pull request with its tests, its `docs/plane-format.md` change first where
it writes a new key, store or lock file, and a changelog fragment where a person would notice.
Review round 2 split the oversized tickets (R24), so a store ticket moves at most three or four
stores. The numbers below are the tickets' keys; the brackets are blockers. The tests each
ticket owns are named in §3.

**The store and its surfaces**

- **0. Land the ADR, CONTEXT.md and this note** []. **16. The amendments** [0]: ADRs 0122, 0069,
  0112, 0053, 0073 §5, 0034 and 0067 (R14), one file each.
- **1. The whole-file store, its contract harness and a toy store** []: the declaration with
  every field of §1, the registry, `read` with its seven answers, `update` and `update_held`
  with the declared lock, kept and refused unknown keys, the damage rule with its exclusive
  create (R14), Set it aside by store id (B-F13), starting fresh grants more (R13), TooOld, the
  version history and the private write. Tests 1 to 4, 6 to 8, 23 to 25, 27, 30, 31 and 57.
- **1b. Rows and files of one record each** [1]: skipped rows, torn lines, pruning and the bound,
  line caps. Tests 28, 29 and 64 (C5).
- **2. The refused-records Notice** [1]: `records-unread`, keyed by the set of refused stores,
  with TooNew, Unplaceable and Unreadable lines and their remedies, and Damaged and TooOld lines
  (E14); the window asks the core for the standing of every registered store on start, on focus
  and after a press (C19). Tests 26 and 58.
- **2b. The doctor's lines, and Set it aside from the terminal** [2]: a line per refused store,
  skipped rows and kept copies counted and named, no finding listed twice (E23), and the
  terminal route (D-F10). Test 59.

**The approval records, then the ledger**

- **3a. Page and Regenerate records** [1, 2]: rows 2 and 3, one shared named lock (D-F6). Tests
  5, 12 and 56.
- **3b. The extension record** [1, 2]: row 1 through the registry (C7).
- **3c0. The sandbox denies a held record's siblings** []: R14, before any copy can exist beside
  rows 4, 5 or 6. Test 50.
- **3c. Harness-declaration and local-profile records** [1, 2, 3c0]: rows 4 and 5, a named lock
  in the session host's folder (B-F6), must not be tracked (R15). Test 38.
- **4. The ledger and its first three adapters** [3a, 3b, 3c]: no change of behaviour. Tests 20
  and 44.
- **7. The Regenerate gate** [4]: one core function, an allow-list (C11, B-F9). Test 15.
- **5. One lifecycle** [4, 7]: unpin and rename both parts, the ledger's two-part approve under
  one lock with its crash order, whole-entry replace, lone parts New (C10). Tests 13, 16, 19, 37
  and 45.
- **5b. The template stamp and the Regenerate framing** [5]: `pf` first, B1 and R8, a changelog
  fragment. Tests 14 and 35.
- **6. Removing a workspace forgets** [5]. Test 46.
- **8. The reach** [4]: lines and digest (R12), the baseline compiled in, the plane grant bound
  to it, `reach-changed` in the bar and the CLI (C12, E26). Tests 17, 18 and 39.
- **9. Forget on the harness card** [4] (R22). Test 54.
- **10. A moved extension** [4] (Q13, B-F17). Test 36.

**The code-approval Notice**

- **11a. The Notice alone** [4]: the family, Approve optional (C14), the cause key's digest and
  `KEPT` (R20), the labels (E16), every adapter's how-it-runs sentence (E19). Tests 21, 41 and 51.
- **11b1. A page's script on the Notice** [11a, 5, 8]. Test 52 (C24).
- **11b1b. Its Regenerate on the same Notice** [11b1, 5b, 7]. Tests 42 and 47.
- **11b2. Extensions on the Notice** [11a]. Tests 22 and 53.
- **11c0. A Blocking ask of no chat** [] (C13). Test 60.
- **11c. The harness start flow** [11a, 11c0]: held (R21), Not now, the journey (E18). Tests 40,
  61 and 65.
- **12. The `mcp` adapter** [4, 11a, 3c]: row 6 onto the store and its Notice. Tests 43 and 62.

**The other stores** (each: test 5 first, the harness switched on, test 25 for a ✱ store)

- **13a. Dispatch queue rows** [1b, 2]: row 8 kept, the drain stops at a skipped message (R16),
  every remover. Tests 9, 32 and 63.
- **13a1. The key's three answers and `refused/`** [13a]: R17, Q15. Tests 33 and 34.
- **13b. Spool lines and the spool's keys** [13a1]: rows 10 and 49. Tests 11 and 49.
- **13c. Skill grade logs, resume files, skill levels and arrived files** [1b, 2]: rows 11, 12,
  19 and 18 (A-F1, C6).
- **13d. The event log** [1b, 2]: row 13 (C8).
- **13d1. The kill switch** [1b, 2]: row 14 (R18). Test 48.
- **13d2. Runner hours, network blocks, the save journal and a LOCAL work log** [1b, 2]: rows 15,
  16, 17 and 58.
- **14a. Forge accounts, budget and etags** [1, 2]: rows 24, 20 (R19) and 21, and the forge's own
  writer onto `rewrite::replace`. Test 10.
- **14b. The machine store** [1, 2]: row 22.
- **14c. Harness profiles, onboarding, layout and runner log cursors** [1, 2]: rows 23, 25, 26
  and 27 (C7).
- **14c1. Theme choices and the report stores** [1, 2]: rows 28 to 31.
- **14d. Push remotes, signing and link hosts** [1, 2]: rows 32, 56 and 7 (C27).
- **14d1. The runner family and window names** [1, 2]: rows 33 (R13) and 34.
- **14d2. The Inbox family** [1, 2]: row 36.
- **14d3. Sandbox grants and the skill edit records** [1, 2]: rows 45 and 51.
- **14d4. The host's stop record, fixed in place** []: row 35 (R23). Test 55.
- **14e. Dispatch records, message records and ended dispatches** [1b, 2]: rows 9, 37 and 38.
- **14e1. Blocked while away, dispatch nevers and the runner's waits** [1, 2]: rows 39, 40 (R13)
  and 50.
- **14e2. Closed chats, reopen and studio chats** [1, 2]: rows 41 to 43.
- **14e3. Secret asks, browsers and mirrors** [1, 2]: rows 44 and 46 to 48.
- **14e4. The skills lock, plane push and save branch** [1, 2]: rows 52 and 54.
- **14e5. Rebuildable caches** [1, 2]: rows 53, 55 and 57.
- **15. Remove the old private writers** [2b, 3a, 3b, 3c, 12, 13a, 13a1, 13b, 13c, 13d, 13d1,
  13d2, 14a, 14b, 14c, 14c1, 14d, 14d1, 14d2, 14d3, 14e, 14e1, 14e2, 14e3, 14e4, 14e5]: every
  caller that writes a private file that is not a record calls `rewrite::replace` with
  `Mode::Private` directly (A-F7, C3).

Tickets 1, 1b and 4 have no behaviour a person sees. Ticket 2 adds a Notice no one meets until a
store refuses. Ticket 3a is the first that could refuse a person's record, and only one written by
a newer purlis or one that is not a record. Tickets 5 to 12 are the ones a person notices most.

## 5. What the order guarantees

- No store can refuse before the Notice that says so exists: every ticket that moves a store is
  blocked by ticket 2, and ticket 14d4's host record is not on the store.
- No key is written before every reader of its store tolerates it: test 5 for rows 2 and 3 in
  ticket 3a, before tickets 5b and 8 write the stamp and `reach`.
- No old re-ask is removed before its adapter draws the Notice: 11a before 11b1, 11b1b, 11b2 and
  11c.
- No adapter answers through the ledger before its record is on the store: tickets 3a to 3c
  before 4, and ticket 12 moves row 6 in the same pull request that adds its adapter.

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

## 7. Review round 2

Five reviewers read spec #6706 and tickets #6707 to #6735, one axis each: A integrity, B
security, C buildability, D failure and recovery, E the person and the language. The operator
ruled on thirteen points as R12 to R24; the rest were the steward's to fix. Each row says where a
finding went. Ticket numbers are §4's keys. Ids are each reviewer's own: A's nits are A-N1 to
A-N8, and B's and D's findings are B-F1 and D-F1 onwards.

| Finding | Resolved |
| --- | --- |
| A-F1 | Row 18 to ticket 13c; §1 rows header |
| A-F2 | R12: ADR §6, Decisions; note test 17; tickets 8, 11b1 |
| A-F3 | Ticket 5 blocked by 7; note test 19 |
| A-F4 | Ticket 13a1 adds the three-way key read and 13b keeps it; note test 33 |
| A-F5 | R23: ADR §1, note row 35, ticket 14d4, test 55 |
| A-F6 | Rows 57 (`glstate.json`) and 58 (a LOCAL work log); `repostate.json` and `harness-wiring.json` named out of scope, with the reason |
| A-F7 | Ticket 15's blockers gain 3a, 3b, 3c and 12; non-record callers call `rewrite::replace` directly; the forge writer moves in 14a |
| A-F8 | Ticket 13b: test 25 for `keys.json` |
| A-F9 | Note test 56 (exit 1) in ticket 3a; test 2 pins the error kind |
| A-F10 | Note test 5 and row 33: `pool-setup.json` refuses unknown keys and keeps refusing; the declaration's refuses-unknown-keys option |
| A-F11 | Note tests 21 and 62, ticket 12 |
| A-F12 | ADR §1 locks; note §1 approval records; tickets 3a, 3b, 3c, 12 |
| A-F13 | R22: ADR §5, Decisions (Q9 narrowed); note §2 harness row; ticket 9, test 54 |
| A-F14 | ADR §4, note §2: four adapters with cause-key words, six records |
| A-F15 | Version history field (note §1, ticket 1); "Approve its script first" opens the Notice (ticket 11b1b); the doctor counts and names (ticket 2b); `reach-changed` (ticket 8); each store keeps today's unknown-key answer unless its ticket says otherwise (note §1) |
| A-F16 | R17: ADR §2, note row 8, test 33 |
| A-F17 | Ticket 13c: a newer resume file is skipped and kept; a newer level file is refused |
| A-F18 | R24: note §4 |
| A-N1 | Note §4 says seven answers |
| A-N2 | Spec: "a row it cannot place, or a torn line" |
| A-N3 | Spec story 16 says "the machine store" |
| A-N4 | Ticket titles no longer call rows 37, 43 and 50 Clone state |
| A-N5 | Spec points at the tickets' keys and issues |
| A-N6 | Note test 6: the toy store is outside the registry |
| A-N7 | Edges dropped: 11b2 from 10, 11c from 9 |
| A-N8 | Note test 52 holds `as-of`; ticket 11b1 |
| B-F1 | R13: ADR §2, note §1 (rows 33, 40, declaration field), test 25; tickets 1, 3b, 14d1, 14d4, 14e1 |
| B-F2 | R14: ADR §1, §2, What this changes (ADR 0067); note new stores, tests 23 and 50; tickets 1, 3c0, 16 |
| B-F3 | Note test 35, three cases; ticket 5b |
| B-F4 | R12: ADR §6; note tests 17 and 52 |
| B-F5 | R15: ADR §5; note §1, test 38; tickets 3c, 12 |
| B-F6 | ADR §1 locks; note §1; tickets 3c, 12; test 54 |
| B-F7 | Note tests 44 and 47; tickets 4, 11b1b |
| B-F8 | ADR §7; note tests 60 and 61; tickets 11c0, 11c |
| B-F9 | ADR §8; note §2 page row, test 15; ticket 7 |
| B-F10 | Ticket 5 blocked by 7 |
| B-F11 | ADR §6; note test 18; ticket 8 |
| B-F12 | ADR §6; note test 39; ticket 8 |
| B-F13 | ADR §1; note test 24; tickets 1, 2. The terminal route of D-F10 is kept, and refuses in a chat |
| B-F14 | R16: ADR §2; note tests 32 and 63; ticket 13a |
| B-F15 | ADR §2; note new stores, test 34; ticket 13a1 |
| B-F16 | R17: note row 49, test 49; ticket 13b |
| B-F17 | Note test 36; ticket 10 |
| B-F18 | Note tests 26 and 41; tickets 2, 11a, 13a1 (the list's lines) |
| B-F19 | Note test 37 |
| B-F20 | Ticket 11b1b: "the fingerprints and the reach shown" |
| B-F21 | Ticket 8: the reach travels with the served standing and is handed back |
| C1 | R12 |
| C2 | R23 |
| C3 | As A-F7 |
| C4 | R22 |
| C5 | Tickets 1 and 1b. Test 31 stays in ticket 1, because rebuild over is a flag on any shape and tickets 14a and 14e5 need it with the whole-file store |
| C6 | As A-F1 |
| C7 | Tickets 14c to 14e5, at most four stores each; titles fixed |
| C8 | Tickets 13d, 13d1 and 13d2 |
| C9 | R18 and R19, written into tickets 13d1 and 14a |
| C10 | Tickets 5 and 5b; ticket 11b1b calls ticket 5's approve |
| C11 | ADR §8; note §2, test 15; ticket 7 |
| C12 | Ticket 8: the bar's word, the CLI word and a changelog fragment |
| C13 | Ticket 11c0; ticket 11c names the three start flows |
| C14 | Ticket 11a (Approve optional); ticket 12 (`COPY_ONLY`); note §2 MCP row |
| C15 | Edges dropped |
| C16 | Row 9 moves to ticket 14e; ticket 13b: test 49, test 25 and the two-version declaration |
| C17 | Note test 5 |
| C18 | Note test 5, row 33; ticket 14d1 |
| C19 | Note §1 registry; tickets 1 and 2 |
| C20 | As A-F9 and A-N6 |
| C21 | The common block of every ticket that may move a recorded fixture |
| C22 | As A-F14 |
| C23 | Note test 44 |
| C24 | Tickets 11b1 and 11b1b |
| C25 | Note test 21: seen red with a fixture |
| C26 | Tickets 2 and 11a: baselines come from the pull request's `stories` run |
| C27 | Ticket 6 enumerates the workspace's pins; row 7 moves with ticket 14d, apart from the machine store |
| D-F1 | R17; note rows 8 and 49, tests 33 and 49 |
| D-F2 | Note row 8, test 9; ticket 13a |
| D-F3 | R16 |
| D-F4 | R18; note row 14, test 48; ticket 13d1 |
| D-F5 | Note test 39; ticket 8 |
| D-F6 | Note §1, test 30; ticket 3a |
| D-F7 | ADR §1 locks; note §1; tickets 3a, 3b |
| D-F8 | Note test 57; ticket 1 |
| D-F9 | ADR §2; note test 58; ticket 2 |
| D-F10 | ADR §1; note test 59; ticket 2b |
| D-F11 | The common block: the whole harness, any test left out named with its reason; ticket 13b test 25; tickets 13d to 13d2 |
| D-F12 | Ticket 5 blocked by 7 and owns the ledger's two-part approve |
| D-F13 | R19 |
| D-F14 | R15 |
| D-F15 | ADR §10; note tests 45 and 46; tickets 5, 6 |
| D-F16 | As B-F11 |
| D-F17 | Note test 5; tickets 3a, 5b, 8 |
| D-F18 | Note row 22; ticket 14b |
| D-F19 | ADR §1, the lost-update sentence |
| D-F20 | Note test 29; ticket 13d2 |
| E11 | R20: ADR §7; note tests 26, 51, 52 and 53 |
| E12 | ADR §7; note §2 page row, test 52; ticket 11b1 |
| E13 | R15 |
| E14 | ADR §2; note test 58; ticket 2 |
| E15 | ADR §7; note §2 extension row, test 53; ticket 11b2 |
| E16 | ADR §7 labels; ticket 11a; spec |
| E17 | R21 |
| E18 | ADR §7; ticket 11c, test 61 |
| E19 | ADR §7; note test 41; ticket 11a |
| E20 | ADR §7; note test 52; changelog fragments in tickets 2, 3a, 3b, 3c, 6, 8, 10, 11b1, 11b2, 12 |
| E21 | Note §1 declaration field; tickets 1, 2, 2b; spec stories 1 and 17 |
| E22 | Spec story 1; ticket 3a's changelog fragment |
| E23 | Note test 26; ticket 2b |
| E24 | Ticket 2: Update purlis opens Settings › Updates and is not drawn when nothing newer is found |
| E25 | Spec, Sentences; tickets 2, 7, 8, 11a, 12, 13a1 |
| E26 | Ticket 8 (`reach-changed`); ticket 2b (names) |
| E27 | ADR §7; note test 52; ticket 11b1 |
| E28 | CONTEXT.md (Private record, Code approval); ADR, this note and the spec say "copied aside", "set aside" and "kept copies"; four adapters; "refuse" against "blocked" in shown text (spec) |
| E29 | Every ticket names its stores and spells out each code at first use |
| E30 | Note test 44 |
| E31 | ADR §2: the Unplaceable sentence ends with its remedy |
| E32 | Note test 26; ticket 11a |
| E33 | Spec and ticket 11c write "Asks (✋)" |
| E34 | Spec stories 26 and 29 shortened |
| E35 | Ticket 0 updates the spec's first paragraph and Further Notes |
| E36 | Note test 42; spec story 49 |
