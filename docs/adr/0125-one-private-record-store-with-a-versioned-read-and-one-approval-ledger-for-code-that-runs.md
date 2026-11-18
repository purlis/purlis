# One private record store with a versioned read, and one approval ledger for code that runs

**Proposed 2026-11-15**, for epic #6677. It follows the operator's rulings of 2026-11-15 on that
epic: **R1** (build A, the record store, with B, the approval ledger, on top of it; C and D are
later efforts), **R2** (a newer record is never deleted or overwritten), **R3** (one subject and
one lifecycle for a page and its Regenerate), **R4** (gaps 14 and 17 are in scope) and **R5**
(two review rounds before building). A, B, C and D are the four candidates of the architecture
report of 2026-11-15, each named where this record first uses it. Those rulings are decided and
this record does not reopen them.

The choices the rulings left open were put to the operator one question at a time on
2026-11-15, as Q1 to Q16. Review round 1 (five reviewers, one axis each) then found what the
draft got wrong, and the operator ruled on the findings that needed a ruling, as **R6 to R11**.
Review round 2 read the spec and the tickets built from this record, and the operator ruled on
thirteen more points, as **R12 to R24**. Review round 3 read the folded spec and tickets, and the
operator ruled on four more, as **R25 to R28**, and then on the five points the fold left open,
as **R29 to R33** (both 2026-11-16). Review round 4 read the next fold, and the operator ruled on
three more, as **R34 to R36** (2026-11-16), and then on the one point it raised, as **R37**
(2026-11-17). Review round 5 read the fold after that, and the operator ruled on two more, as
**R38 and R39** (2026-11-18). Each is marked (Qn) or (Rn) where it applies and listed under
[Decisions](#decisions-on-q1-to-q16-r6-to-r11-and-r12-to-r39). The design note's §6 to §10 say
where every finding of rounds 1 to 5 went. A finding id such
as B-F6 or E28 is a review reviewer's (A to E) own number, in the round the note's section names.

The background is [`docs/surfaces.md`](../surfaces.md): gaps 1, 2, 7, 10, 11, 12, 14, 16 and 17,
and seams 1 and 2 (its name for a place where one shared piece would remove the most
inconsistency). The call-site inventory, the contract tests and the order of tickets are in
[`docs/design/record-store-and-approval-ledger.md`](../design/record-store-and-approval-ledger.md).
Every `path:line` below was read at `2dc0d73c2`. A line moves as the code changes, so the
function named beside each one is the anchor that lasts.

## Where purlis was

**Every private record reads its own version, its own way.** purlis keeps about forty private
JSON and JSONL records in the Machine and Clone-state tiers (ADR 0069). The core has 36 version
constants, and each store has its own hand-rolled bounded read. A record of a version the store
does not know gets one of five answers, depending on the store (gap 1):

- **Refused whole.** The extension record refuses it (`extension.rs:2473`, in `read_record`) and
  will not overwrite it (`extension.rs:2624`, `update`).
- **Copied aside, then overwritten.** The machine store reads a file of another version, or one
  that does not parse, as nothing (`machine.rs:2456`, `Dropped::TheStore`). Its next update
  copies the bytes to `machine.json.unread` and writes a fresh store over the file
  (`machine.rs:2561`, `update`; `:2595`, `keep_unread`), as the ruling of 2026-11-08 in
  `docs/plane-format.md` has it. Only a file it cannot read at all (a link, too large, no
  permission) is never overwritten.
- **Skipped and left on disk.** The dispatch record and the skill grades.
- **Deleted without a word.** A queued dispatch message of another `v` is removed and never
  delivered.
- **Downgraded on write.** `forge-accounts.json` reads its `version`, ignores it, and stamps `1`
  on the next write.
- **Written and never read.** `artifact-approvals.json` and `artifact-regenerate-approvals.json`
  write `"version": 1` (`artifact/approval.rs:432`, in `change`) and never look at it again. A
  file of either that does not parse reads as empty, and the next approval writes only itself.
  The two harness approval records under `.charter/` carry no version at all
  (`profiletrust.rs:133`, `read_record`).

A garbage version also reads as a missing one (gap 2). Every one of these reads is a copy of
the same idea, and the doctor carries a further copy of the spool `keys.json` check
(`doctor/spool.rs:50` against `hookwire/spool.rs:981`).

**There is one placement already, for one file.** `compat::Schema` (`compat.rs:19`) sorts the
project file's `schema` into `Understood`, `TooNew` and `Unplaceable`, and `Schema::refusal`
(`compat.rs:31`) gives the one sentence `doctor`, the forge commands and `init` all say. It is
used for `charter.toml` alone, and it reads a negative integer as `Understood`
(`compat.rs:55`).

**Private writes go through one mechanism under three names.** `machine::write_beside`
(`machine.rs:2717`) and `plane::write_private` (`plane.rs:466`) are each one line that calls
`rewrite::replace` (`rewrite.rs:554`) with `Mode::Private`. They differ only in the root the
walk is gated from. `glrefresh.rs:524` wraps `plane::write_private`. Only the forge's own
tempfile-and-persist (`forge/etag.rs:104`, `write_private`, which the forge budget also calls)
bypasses all three.

**Locks are per store, and of three kinds.** The machine store holds `machine::Lock`, a blocking
`flock` on a named file in the session host's folder, which no chat can open (`machine.rs:2632`;
`Lock::named` at `:2640`). `rewrite::Lock` locks a folder: `Lock::on` (`rewrite.rs:238`) blocks
and is only for a folder no chat can open, and `Lock::bounded` (`rewrite.rs:209`) waits ten
seconds at most and then refuses as busy. `rewrite::update` takes the bounded lock, and
`rewrite::update_held` (`rewrite.rs:288`) is the same read-modify-write for a caller that holds
a lock of its own. Some read-modify-writes hold no lock at all, the approval records among them
(`artifact/approval.rs:426`, `change`; `extension.rs:2624`, `update`).

**Four approvals of code that runs, each on its own.** Each adapter has its own Standing or
Approval enum, key, record, validation and re-ask:

| Subject | Standing type | Record and tier | Forget | Carried on move |
| --- | --- | --- | --- | --- |
| Extension | `extension.rs:815` | `<config>/extensions.json`, Machine, device-bound | `extension.rs:2791` | no: a move is a new program (`extension.rs:2738`, `moved_clears_reading`) |
| Page script | `artifact/approval.rs:59` | `<config>/artifact-approvals.json`, Machine, device-bound | `artifact/approval.rs:380`, called on unpin (`artifact/mod.rs:436`) | yes, on a workspace rename (`artifact/approval.rs:393`, called from `wscmd/rename.rs:1298`) |
| Regenerate code | `artifact/regenerate.rs:382` | `<config>/artifact-regenerate-approvals.json`, Machine, device-bound | none | no |
| Harness declaration | `profiletrust.rs:244` | `.charter/harness-declarations-approved.json`, Clone state | none | not applicable: the record is in the clone |

So the page and its Regenerate, which share one key (`artifact/approval.rs:106`, `key`, read
by `artifact/regenerate.rs:455`), part ways. Unpinning forgets the first and keeps the second,
and a rename carries the first and loses the second (gap 11). The Regenerate fingerprint frames
the code folder's absolute path (`artifact/regenerate.rs:288`, in `fingerprint`), so nothing
could carry it across a rename without computing it again. A Regenerate run checks only its own
standing, never the page's (`app/src-tauri/src/regenerate.rs:232`, `run_pressed`), so it runs
for a template nobody approved and writes what that template declares (gap 16). An approval
binds bytes, not what the declared queries reach, so a later purlis or extension that lets a
query read more is still covered by the old approval (gap 17). The re-ask is drawn five ways
(gap 14; the surfaces page counts four, and the harness declaration's start dialog is the
fifth):

- the extension dialog, whose re-ask is "Trust it anyway" (`app/src/ApproveExtension.tsx:59`,
  the button at `:73`);
- a sentence in a `<span>` in the page bar (`app/src/ArtifactTab.tsx:236`, `ScriptStanding`);
- the Regenerate `AlertDialog` (`app/src/ArtifactRegenerate.tsx:144`);
- the harness profile's start dialog (`app/src/StartApproval.tsx:78`, `StartApproveAsk`);
- and, for a dispatch grant, which is reach and not code, a `Notice`
  (`app/src/DispatchGrantNotice.tsx:169`).

## The decision

### 1. One record store for private records

**Every private JSON record purlis keeps in the Machine tier or in Clone state is read and
written through one module, the record store** (`purlis_core::record`; the name is the
build's to choose). This is A. It serves those two tiers and no others. A Plane file keeps
`compat::read`, because the project's schema governs it. A Keyring item is not a file.

**A store is declared, not hand-written.** Each store names, in one place:

- its **root**, apart from its tier: the config home, `<data>`, the session host's folder, the
  plane's `.charter/`, or the trust root a state folder is gated from (`hookstate::trust_root`,
  which `mcp-approved.json` uses). A tier does not decide a root: the spool's messages sit under
  `.charter/` and are Machine-tier;
- the file, as a path under that root;
- its **tier and marks**, the same words as its `**Tier:**` line in `docs/plane-format.md`
  (ADR 0069), and whether it is **rebuildable** (R9);
- its **shape**: **whole file** (one JSON document, replaced whole), **rows** appended to a
  JSONL file, or **one file per record** in a folder, each made once;
- its **version key** (`version`, `v` or `schema`), the version it writes, the oldest it reads,
  and a migration for each version below its own that it reads (§2); a store that reads more than
  one version today declares each, so a file today's reader takes is still read (D-F3, round 5);
- its **bound** in bytes: today's, or, for a store that has none, one well above what its feature
  can write, with what the feature does when a write would pass it (D-F5, round 5);
- its **lock** (below);
- its **serializer**, which stays what the file is written with today (Python's `indent=2`, with
  or without `ensure_ascii`, sorted keys, or serde's pretty print), so no file's bytes change
  when its store moves;
- whether it **keeps unknown keys**, and at which levels (about 45 of the 56 stores do not
  today);
- whether it **must not be tracked** by git (B5, §5);
- its **fail-closed answer** (§2) and its **busy sentence** (below);
- whether **starting fresh grants more** (R13, §2): a store whose fresh, empty record would let
  through what the person refused;
- what **starting fresh costs**, in one sentence the person reads before Set it aside (E21);
- for each version it knows, the release that introduced it (below);
- its **write kind**, which keeps the writer the file has today: private, secret
  (`rewrite::Mode::Secret`, which writes nothing where the filesystem cannot hold `0600`; the vault
  links, the keyring vaults' indexes and the no-seed marks, C-F5, round 5),
  through a same-user folder handle (`purlis_same_user::PrivateDirectory`), or pinned to the bytes
  read (`proposed::write_pinned`) (B-F3, C-F4, A-F4, round 4);
- whether it is **this user's alone**, which keeps today's owner-and-mode check
  (`purlis_same_user::read_private_file`): such a store's read answers Unreadable for a file not
  owned by the effective user or with any group or other bit, as today, and never judges the
  folder; its write goes through the same-user folder handle, which refuses a folder another user
  owns and tightens one of this user's to `0700`, as today (B-F3, round 4; C-F4, round 5);
- where its **kept copies** go, when the sandbox holds its file by exact path (below, R25 and
  R35).

Every declaration is in one registry, with how a path that has a key in it (per chat, clone or
account) is enumerated, so the window and the doctor can list every store (C19). The window's
sweep reads, among keyed stores, only a lockout; the doctor counts what the others refuse (C-F6,
round 4).

**One record is left out, by name** (R23). `runners-stopped.json`, the host's record of which
runners its stop reached (`crates/host/src/reach.rs:53`, `RUNNERS_STOPPED`), stays out of the
record store, because the host crate does not link `purlis-core` (`crates/host/Cargo.toml:14`).
Its versioned read is fixed in place in the host: it refuses a newer version and never
overwrites. A stop or a re-arm pressed while it is refused still acts and is held in memory;
only the save is refused (C-F14, round 3). The host's start sends its stop again, which keeps a
lost `owed` set safe (`crates/host/src/reach.rs:111`). A lost `reached` set is not: a runner
this host's stop already stopped answers "stopped already" on the second send and is not
re-armed with it (`reach.rs:191`). So while the record is refused or damaged, a host that starts
stopped counts a runner that answers "stopped already" as reached, and the re-arm reaches it
(R33). The core's doctor reads the record's version itself, which needs no link to the host, and
prints a line with the file's path and how to move it aside by hand (R33); the session host's log
says the same in the TooNew sentence's words.

**A second record is left out, by name** (R29). `<config>/purlisd/hosts.jsonl`
(`crates/host/src/hostrecord.rs:45`, `HOST_RECORD`), the session host's note of the programs it
started, is in the same crate, so it is fixed in place in the host too: it is a record of rows, and
every write of the file whole (`take_over` at each host start, `empty` at a clean exit, and the
rewrite past 256 KiB, all through `start_again`) carries a line it cannot read through byte for
byte, where today each drops it, so a newer host's line survives an older host. A carried line
always ends with a line end before the host's own lines. A file the host could not read is never
emptied; the host keeps its notes in memory and says so in its log (D-F1, C-F2, round 4), and since
such a file no longer heals itself at the next start, the core's doctor prints a line for it, as
for the stop record (D-F6, round 5). Round 3's sweep of `docs/plane-format.md`
found other private records the inventory never listed; the core's move onto the store in tickets
of their own (the design note's rows 59 to 67, and 69 to 71 from round 4's sweep, A-F5), and
`<git dir>/purlis-op.json` and `purlis-resolve.json` are the repository's journals, not purlis's
records, and stay out (R29).

**The read places the version as `compat::Schema` does, with two answers more.** The store
reads the document, or each row, and places its version key:

- **Understood:** absent (the store's unstamped version), or an integer from its oldest to its
  own. A version below its own is migrated on the read.
- **TooOld:** a positive integer below its oldest. It is its own placement, never Unplaceable
  (§2). For a store where starting fresh grants more, its default migration follows R13: refused,
  and set aside only on a press (A-N7, round 3).
- **TooNew:** an integer above its own.
- **Unplaceable:** anything else: a string, a float, zero or a negative number. This is its own
  answer and its own sentence, never "says no version" (gap 2). Unlike `compat::schema`, a
  negative integer is not Understood.

Before it places the version, the read sorts out a file it cannot use:

- **Unreadable:** a link, a file over its bound, a file that is not a regular file, or one this
  process may not read; and, for a store that is this user's alone, a file another user owns or
  may read or write. Nothing about the bytes is known, so the file is never touched.
- **Damaged:** bytes that are not JSON, or not the store's shape (a whole-file store whose
  document is not an object). A row store's line of that kind is a skipped row (§2).

Today's bounded reads keep their bounds; they move into the declaration. A store with no bound
today gets one well above what its feature can write (D-F5, round 5).

**The interface:**

- **`read`** answers `Understood(value)`, `Absent`, or one of `TooOld`, `TooNew`, `Unplaceable`,
  `Unreadable` and `Damaged`, each with its sentence (§2).
- **`update`** is a locked read-modify-write. It holds the store's declared lock across the read
  and the write. It writes only over `Understood` or `Absent`, and over `Damaged` after the copy
  aside (§2), unless starting fresh grants more. It never writes over any other answer. A write
  over the store's bound refuses with the store's sentence, because the next read would be
  Unreadable (D-F8). `update` never nests: a caller that must
  change two files under one lock takes the lock once and calls **`update_held`** for each, as
  `rewrite::update_held` does today.
- **Unknown keys are kept** where the store declares it. A value keeps every key this build does
  not know, at every level the store declares as open, and writes them back unchanged. A newer
  purlis that added a key without bumping the version loses nothing to an older one. An entry
  that `update` replaces on purpose is replaced whole, unknown keys and all (B4, §4).
- **Replace a Damaged record on the person's press** is one more call, for a store where starting
  fresh grants more whose person's own press repairs a Damaged record, as the Skill studio's Accept
  does for the level records. Under the store's lock it reads again, goes on only while the answer
  is still Damaged, copies the bytes aside by the damage rule, then writes the value the caller gives
  in one rename, so the record is never absent between the two. It never runs over TooNew or
  Unplaceable, and its callers are listed with Set it aside's (C-F1, round 5).
- **`remove`** is a store's own removal of a whole record, for its lifecycle code and its
  collectors. Under the store's lock it reads the record and removes it only when it is
  Understood; a record it cannot read is kept, and the doctor counts it. Only a store declared
  transient or rebuildable may remove a whole record whatever its version, as R9 and R32 allow
  (R36; C-F5, A-F6, round 4).
- **The private write is one function**, `rewrite::replace` with `Mode::Private` (or
  `Mode::Secret` where the declaration says so), gated from the store's root; a store declared to
  write through a same-user folder handle or pinned to the bytes read keeps that writer, inside
  the store. A store's own code never calls any of them.

**The lock is declared per store, never one lock for a tier** (A5, D8):

- a store in a folder no chat can open (the config home, the session host's folder) keeps a
  blocking lock. The machine store keeps `machine::Lock::named` and its lock file, so an older
  and a newer purlis still exclude each other;
- a store in a folder a chat can open (the plane's `.charter/`) takes a bounded lock, or a named
  lock in the session host's folder, as `rewrite::update_held`'s callers do;
- **every approval record**, wherever it lives, takes a named lock in the session host's folder,
  which no chat opens (`machine.rs:2626`), and waits on it with a bound. A chat can then never
  hold it, and a stopped process can never freeze every approval (B-F6, D-F7). The page subject's
  two records share one lock from the first ticket that locks them, so builds on either side of
  the lifecycle work exclude each other (D-F6). A lock file's name is a contract between builds:
  it is recorded in `docs/plane-format.md` and pinned by a test;
- a store with a lock of its own today keeps it: the event log's lock, and the compare-and-swap
  of `skills.local.lock`;
- a store with no lock today gets one, and a lock this effort adds waits with a bound wherever
  the store lives, so no write that never waited before can now hang. Its ticket names its lock
  file in `docs/plane-format.md` and its busy sentence, and switches on test 30's bounded case
  (D-F13, round 3). An older purlis's write of it still takes none, so while
  an older and a newer purlis both run, a lost update stays possible. For an approval record a
  lost approval fails closed and asks again, but a lost forget does not: the older build's
  whole-file write puts the forgotten entry back, for the bytes once approved (D-F19);
- where a lock cannot be taken (a filesystem with no `flock`), the update goes ahead unlocked,
  as `machine::Lock` does today and for its reason: refusing would lose every write, not one.
- A bounded lock that is still held after its wait refuses with the store's **busy sentence**.
  `rewrite`'s own sentence ("a chat in this project may be holding it … close that chat") is
  right only for a folder a chat can open. The declared sentence is only its first half, and the
  caller ends it with what did not happen, then "Try again." (E42, round 3). An approval record's
  first half is "purlis is still recording another approval.", so an approve says "… Nothing was
  approved. Try again.", an unpin "… Nothing was unpinned. Try again.", a workspace's removal
  "… The workspace was not removed. Try again." and a Forget "… Nothing was forgotten. Try
  again." Any other store's first half is "purlis is still recording {what}.", ended by "Nothing
  was changed. Try again."

**The copies are made so a chat cannot aim them** (R14, narrowed by R25). The copy aside and
the set-aside rename are each an exclusive, no-follow `0600` create through the gated walk from
the store's root, with a counter suffix when the name is taken, so a planted link or file is
never written through or replaced. If the copy cannot be made complete, the update refuses and
the damaged bytes stay (D-F8). A store's kept copies sit beside its file (R6), with one exception
(R25): the sandbox can deny only an exact path or a whole folder, and Claude Code's own Linux
sandbox holds no pattern (`core/sandbox.rs:1324`), so no name prefix can be denied. The kept
copies of the three records the sandbox holds (the harness-declaration, local-profile and MCP
approval records, rows 4, 5 and 6) therefore go into a folder the sandbox already denies whole:
`<state folder>/app/kept/`, inside the `app` folder that ADR 0067's class 2 denies to every
chat's writes under every spelling of the state folder (`core/sandbox.rs`, the `app` denial).
Where `$PURLIS_HOME` puts the state folder outside the plane, the same `app/kept/` folder there is
denied whole beside the MCP record's own denial, and made `0700` before the first chat's sandbox is
compiled (B-F5, round 4). Their locks are in the session host's folder, as every approval
record's is. Each backend's compiled profile is shown to refuse a write there (test 50). The ADR
0067 amendment for a name-prefix denial is dropped (R25).

**R35 widens R25 to every store the sandbox holds by exact path, for reading or writing**: its
copies and set-asides go into a folder held whole with at least the same access. `vault-links.json`
(denied for reads and writes by its exact path, `core/sandbox.rs`, the vaults block) keeps them in
`<state folder>/vaults/kept/`, inside the vaults folder, denied whole for reads and writes; and
`runners.json`, `runners-left.json` and `peers.json` (each denied for reads and writes in the
config home) keep them in `<config home>/purlisd/kept/`, inside the session host's folder, denied
whole the same way. `app/kept/` would not do for them, since `app/` is denied for writes only.
`runner-hours.jsonl` is held by exact path too, but a row store makes no kept copies (B-F2, round
4).

A file a commit could have brought is not a kept copy (B-F5, round 3). For a store declared must
not be tracked, a file in its kept-copies place that git tracks or does not ignore is named by the
doctor as a file git could have brought, which purlis did not make, and the doctor never says how
to put it back. The check is the record's own (§5). A record R38's cure moved into the kept folder
(`<record>.git-brought-<UTC>`, §5) is named the same way, by its name, since the kept folder is
ignored (B-F1, round 5).

**Set it aside is asked for by the store, never by a path** (B-F13). The window's press and the
person's terminal (D-F10) name a declared store by its short id, and for a store keyed per chat,
clone or account, by the key the registry enumerated for that file, never a path (D-F12, round
3). Under the store's lock it reads again, and refuses unless the answer is still one Set it aside
is for. The rename is a no-replace rename; where the filesystem has none, it is a hard link then
an unlink, and where neither holds it refuses with the store's sentence, never a plain rename
(D-F17). The terminal route is a fix of the doctor's, `purlis doctor --fix set-aside:<id>`
(`<id>/<key>` for a keyed store), and the doctor prints that id beside each refused store's line
(C-F16, E49). It refuses when run from a chat, as `purlis persona approve-mcp` does; that check
reads the environment, so the boundary that holds is the sandbox's denial of writes to every
store marked starting fresh grants more (`.charter/app/`, the config home), and no such store
is ever kept in a folder a chat writes (B-F11). No purlis MCP tool sets a record aside.

**What the refusal says.** No new key is added (Q1). An older purlis cannot know which release
wrote a newer version, so a sentence never names "purlis X.Y or later". The declaration records,
for each version the store knows, the release that introduced it. That history goes into
`docs/plane-format.md` and into the detail of a TooOld refusal. A version first written by a
development build records that build's version until a release ships it (D13). Every refusal is
the same in the doctor, the CLI and the window. A command that meets one exits 1, as every
refusal does (ADR 0120), never 70.

### 2. Newer, older and damaged records

R2 decides what a newer record gets, and R6 what a damaged one gets. This section says where
each lives, and the sentence each one says. A sentence leads with what stopped working. The
path, the version found and the versions this build reads go in its detail, not in the
sentence. `{what}` is the store's own name for itself, such as "your extension approvals".

- **TooNew, whole file: refused, kept, and never written over** (R2). "purlis cannot read
  {what}. A newer purlis wrote them, and this one is {version}. Nothing was changed." The
  remedies say the rest, so the sentence stays true when Update purlis is not drawn (E54, round
  3). Its first remedy is Update purlis, and its second remedy is **Set it aside and start fresh** (R6): on the person's press,
  purlis renames the file beside itself to `<file>.set-aside-<UTC time>`, never replacing a file
  already there, and the store starts empty. That is the way out after going from a development
  build back to the stable one, where Update purlis leads nowhere (D1). Nothing is deleted.
- **TooOld: migrated.** A record older than the oldest its store reads was written by an older
  purlis, so R2 does not hold it. Its store declares its migration. The default migration is the
  damage rule below: copy it aside, then start fresh. Its sentence: "purlis no longer reads {what}
  in the form an older purlis wrote them, so it copied them aside and started fresh. Nothing was
  deleted." No store has a TooOld version on the day this ships (D3).
- **Unplaceable: refused, kept, and never written over.** "purlis cannot read {what}: their
  version is not one any purlis writes. Nothing was changed. Set them aside to start fresh." Its
  one remedy is Set it aside and start fresh, on the person's press (E31). Update purlis is not
  offered, because nothing says a newer purlis would read it. See [New
  questions](#new-questions-for-the-operator-decided), N1.
- **Damaged, whole file: copied aside automatically, then the store starts fresh** (R6). This is
  the machine store's rule of 2026-11-08, made every store's. The read answers `Damaged` and the
  feature reads nothing. The next `update` first copies the bytes to `<file>.unread`, or to
  `<file>.unread-<UTC time>` when one is already there, never replacing it, and then writes.
  Before the copy, its sentence is "purlis found {what} damaged. It copies them aside and starts
  fresh the next time it changes them. Nothing was deleted."; after it, "purlis found {what}
  damaged, so it copied them aside as {file name} and started fresh. Nothing was deleted." (E41,
  round 3). A crash between the copy and the write leaves a second, identical copy at the next
  update; that is accepted, since nothing is lost (D-F15). The machine store's `.unread` stops being one copy, the latest: R6
  deletes nothing, and since every older build replaces the bare name, this build writes the
  machine store's copies only under the timed name (D-F18). "Copied aside" is the automatic
  rule's word, "set aside" the press's, and the two kinds of file are the store's **kept copies**
  (E28).
- **Damaged, where starting fresh grants more: refused, like Unplaceable** (R13). For a store whose
  fresh record would let through what the person refused, the fail-closed answer holds and writes
  refuse. Its sentence is "purlis cannot read {what}: they are damaged. Nothing was changed. Set
  them aside to start fresh." (E41, round 3). Set it aside and start fresh is offered on the
  person's press, and its cost line names what it lets through, not only what is forgotten (E71,
  round 5). Where the person's own press already repairs such a record today (the Skill studio's
  Accept, for the level records and `unaccepted.json`), that press stays the remedy for Damaged and
  writes through the store's call that replaces a Damaged record on the person's press (§1), so
  "writes refuse" holds for every other writer (C-F1, round 5). This holds for
  `.charter/app/dispatch-never.json`, the one deny among dispatch's records, and for
  `runners-unsandboxed.json`, whose fail-closed answer is that every runner counts as tainted.
  Each store's ticket checks its own record for the same property, and says in its pull request
  what a fresh, empty record lets through or loses that the damaged one held (B-F10, D-F7, round
  3). It holds too for skill levels' `unaccepted.json`, a deny: "what cannot be read is never
  taken as accepted" (R29); and for the level records themselves (`…/skill-levels/<chat>.json`
  and `unnoted.json`), whose code already treats a record it cannot read as naming no folder, so
  every workspace and persona folder is held until the person accepts it (R13; B-F1, C-F3, A-F3,
  round 4). And three stores whose fresh record loses what others rely on, rather
  than granting more, are refused the same way (R31): the runner's `waits.json`, whose fresh record
  makes the next start retire every queued message, and `runners.json` and `runners-left.json`,
  whose fresh record forgets every pool and runner while their containers run. Their Set it aside
  names what is lost. `peers.json`, whose fresh record lets a second pairing through, is refused
  the same way (R13; C-N5, round 5).
- **Unreadable: refused, and never touched.** "purlis cannot read {what}: {cause}. Nothing was
  changed. Make it a plain file purlis can read, or move it aside." These are the words the
  machine store already says for this case (`docs/plane-format.md`, ruled 2026-11-08). Its way
  out is Dismiss and a press that shows the file in its folder.

**What the feature does meanwhile is the store's declared fail-closed answer**, which can only
ask more or do less, never grant more. An approval record that refuses reads as nothing
approved, so the subject asks, and approving refuses with the record's sentence. The design note
gives each store's answer in its own column, and names the stores whose answer locks the person
out of something: the approval records, `machine.json`, `profiles.json`, the spool's `keys.json`,
`secret-asks.json`, the forge budget (R19), skill levels' `unaccepted.json` (R29), the runner's
`waits.json` (R31) and the kill switch (R18) (D-F16, round 3; D-F3, round 4). For those,
apart from the kill switch, whose way out is Re-arm, Set it aside and start fresh is the remedy
that matters; so it is for the stores where starting fresh grants more (E49).

**One Notice for every store that refuses** (R10). After a downgrade, many stores may refuse at
once, so they share one Notice, cause key `records-unread`, which lists each store with its
sentence and its own remedies, plus a doctor line per store. The Notice settles when the last
store reads again. Its cause key is keyed by the set of refused stores, so a dismissal never
hides a store that refuses later (R20). A Damaged store has a line of its own from the read, so a
store read often and written rarely is never silently empty, and the line names the copy once it
is made; a TooOld store's line follows its migration (E14, D-F9). A Damaged or TooOld line stays,
naming its copy, until the Notice is dismissed; it is part of the cause key's set (E41, round
3). The heading's `{n}` counts only the lines of stores purlis cannot read (refused, Damaged or
Unreadable): a TooOld line after its migration, a tracked-record line and the held-message line
are lines purlis can read, and a Notice of only those has no heading line (E69, round 4). The window learns the standing of every
registered store from the core, on start, on focus and after a press (C19). That sweep reads the
whole-file stores that can refuse; a rebuildable store, a row store and a file-per-record store
raise no line of their own except Unreadable, so many keyed files cost nothing per focus (C-F13,
round 3). The window's doctor line leaves refused stores to this Notice, so nothing is listed
twice (E23). When a dispatch message is held (below), the Notice has one line for it that links
to the doctor, which lists each one (R26).

**A row store skips a row it cannot place, and keeps it on disk** (R2):

- For a JSONL file, a rewrite carries every skipped line through byte for byte, in its place.
  For a folder of files, the file is left.
- **A torn last line is a skipped row** (D7). An append always starts on a fresh line, so the
  row it writes never joins a torn one.
- **Pruning and the byte bound never drop a skipped row** (D6). A build that cannot read a row
  cannot know its age, so it keeps it. Skipped rows count toward the bound, and the build prunes
  its own rows first. If the skipped rows alone fill the bound, appending refuses with the
  store's sentence, and the doctor says why. A store that also caps its lines (the save journal
  keeps 500) counts only this build's rows against the cap, and a save-journal append the bound
  refuses stays silent, as today, because the save has still been made (D-F20).
- The doctor counts each store's skipped rows.
- **The event log** (`<data>/events/<device>/`) is a segment being written plus sealed segments
  (`eventlog.rs:700`), and its retention deletes sealed segments whole by age, past
  `retention.hold` even when unread (`eventlog.rs:713`). Its sealed segments are part of the
  store; its writer counts `seq` on past the newest row it can read, taking `seq` from a skipped
  row where it can; a subscriber is told a skipped row as a miss, so no gap is silent; and its
  bound is per segment, no smaller than a segment can grow (D-F8, round 3). Its retention may delete
  a sealed segment whole, skipped rows included: a row in a sealed segment is as old as the segment
  for every build. That is a narrow exception to R2, like R9's, and the pruning rule above does not
  apply to the event log (R32).

**The kill switch fails closed by halting** (R18). Its journal is a row store
(`<config>/kill-switch.jsonl`), and a line this build cannot read may be a newer build's stop
(`halt.rs:139`, `stopped_by_journal_in`, skips such a line today):

- a last line this build cannot read counts as halted, and the window says "Every chat purlis
  started is stopped: purlis cannot read its last stop or re-arm, which a newer purlis may have
  written. Re-arm to let chats start again." (E39, round 3);
- this build's own re-arm, written after that line, ends the halt;
- a stop or a re-arm is never refused for the bound or for the 1000-line cap (`halt.rs:38`,
  `JOURNAL_LINES`), so the kill switch is exempt from the refusing append, and it keeps skipped
  rows past its cap. So that the exemption never makes the journal Unreadable for its size, the
  journal is read from its tail: its last line is placed from the file's end, and its bound applies
  to one line, not to the file (B-F7, D-F2, round 3);
- the journal keeps its replace-by-rename write (`halt.rs:327`), so no reader ever sees half a
  line, which under the first rule would be a halt; the save journal keeps its own write too (D-F2);
- the session host's name for a stop is built from the journal's last line as bytes when that line
  is a skipped row, so two stops this build cannot read, with a re-arm between, are two stops
  (D-F2). The core builds it (`halt.rs:165`, `OnDisk::last_stop`), since the host crate links no
  core (A-N1, C-F8, round 4);
- **one tail read serves every reader, per spelling** (A-N1, B-F6, C-F8, D-F2, round 4): one
  core function answers what a journal ends on, from its tail, for every spelling of the config
  home, and `stopped_on_disk`, `journal`, `last`, the append, `OnDisk::last_stop`, `rearm_by` and
  the window's `look` all call it. Every rule here applies per spelling, and a re-arm is appended
  to each spelling whose journal ends on a stop or on a line this build cannot read. The journal
  gets a lock, in the session host's folder, which no chat opens (B-N1, round 5), and a re-arm holds
  it from the marker's removal until every spelling's line is written (D-F7, round 5); a stop is
  never refused for it, since the marker holds the stop;
- an Unreadable journal (a link, not a regular file, or one this process may not read) with the
  marker present counts as halted. Re-arm then refuses, since its line cannot be written, with the
  Unreadable sentence naming the journal: "purlis cannot read its record of stops: {cause}. Make
  it a plain file purlis can read, then re-arm." (D-F2, E39);
- with two builds running at once, one build's re-arm is never read as tampering by the other.
  **The rule is in the window, not the store** (R28): `look` (`app/src-tauri/src/killswitch.rs:329`)
  decides tampering, and today it restores the marker and records a tamper whenever its switch is
  stopped in memory and the marker is gone, without reading the journal. A window stopped in
  memory that finds the marker gone now reads the journal's end, which is in a folder no chat
  opens:
  - if it ends on a re-arm this build can read, written after the stop this window holds (its `at`
    not before the window's note of its stop, at the journal's one-second grain; D-F7, round 5), and no
    spelling's journal ends on a stop or on a line this build cannot read, the window follows the
    re-arm and runs again;
  - if it ends on a line this build cannot read, the window stays halted with the first rule's
    sentence and writes no tamper line and restores no marker;
  - if the journal's lock is busy, the look decides nothing and the window stays halted until the
    next look, so a look between a re-arm's two writes is never tampering (D-F2, round 4);
  - anything else, an older re-arm included, is tampering, as today.

  This lets `look` move towards running in one case, a re-arm in the journal, which ADR 0071 is
  amended to allow (What this changes, below). It is tested at the window with two switches on
  one config home (test 69).

**A forge budget this build cannot read counts as spent** (R19), for background requests, never
for a request a person is waiting on, as today's rule for a spent hour (`forge/budget.rs:316`,
`admit`). That makes it a lockout store, listed in `records-unread`, and its lock name is kept
(`forge/budget.rs:445`, `lock_name`). A request held back for it says the store's sentence, with
no hour and no turn time, since neither is known (D-F14, round 3).

**The dispatch queue stops deleting** (R2):

- Today `read` (`dispatchqueue.rs:613`) answers "does not check" alike for six causes, and every
  such message is removed (round 3, A-F3, C-F4, D-F4). Under the store each has its own answer:

  | What the message is | Answer | Holds the drain |
  | --- | --- | --- |
  | delivered (at or below the floor) | removed, as today | no |
  | another `v`, or a body this build does not know after its MAC checked | a skipped row: kept, "waiting for a newer purlis" | yes |
  | bytes that are not JSON, or a `seq` that is not its file's | a skipped row: kept, "cannot be read" | yes |
  | over the bound | Unreadable: never touched, "cannot be read" with its cause | yes, since a newer build with a larger bound may read it |
  | MAC fails under a key that was read | moved into `refused/` (below) | no |
  | keys unavailable | waits where it is | yes |
  | keys read, no key for it (R17) | kept, undelivered (below) | no |

  Every path that removes a queued message keeps what this table keeps: the window's drain
  (`app/src-tauri/src/dispatched.rs:1035`, `drain_locked`), the runner host's own drain, `carry`
  (`hookwire/relay/carrying.rs:201`), which carries a chat's queue to the window on a runner (D-F1,
  round 5), the host's retire pass (`dispatchqueue.rs:724`, `drop_unchecked`, which the drains call
  too), an ended chat's `forget`
  (`hookwire/spool.rs:1594`) and the report of an ended chat's queue
  (`dispatchqueue/undelivered.rs:153`, `set_aside`) (D-F2, D-F4). An ended chat whose queue holds
  a kept message keeps its key and its queue, as a chat whose messages could not be set aside does
  today (`hookwire/spool.rs:1601`), so a newer purlis can check the message; a chat given that
  number later gets a fresh queue only once the old one is empty or removed (R30). The key is kept
  on every path that drops keys, the drain's "a chat keeps the key it was issued last" included
  (D-F5, round 4). The messages that check behind a held one are still reported once when their
  chat ends, or moved to its new number when it is started again (`dispatchqueue.rs:428`, `moved`);
  only the held messages stay (D-F2, round 5).
- **The delivered floor never passes a message that holds the drain** (D5). Today a delivery raises
  the chat's floor (`dispatchqueue.rs:630`, against `delivered_floor`), and a message at or below it
  is then dropped as delivered (`hookwire/relay/carrying.rs:402`). **A chat's drain stops at the
  lowest message that holds it** (R16, and the table above), as it already stops at the lowest
  `.part` (`dispatchqueue.rs:583`). Later messages wait, and the floor never rises past it, so the
  floor keeps its second job: no copy of a delivered message is delivered twice. Both drains stop,
  the window's and the runner host's `carry`, and a sender whose fresh message waits behind a held
  one is told it is queued, not delivered, by each (D-F1, round 5). The person can
  remove a held message, whose path is listed, to unblock the queue. There is no format change.
- **Where a held message is listed** (R26). The person's surface is `purlis doctor`: each skipped,
  Unreadable, refused or keyless message by chat, seq and path, what it holds of its queue, and the
  command that removes it. The `records-unread` Notice has a line that links to the doctor. A
  chat's own `purlis dispatch list` (`crates/purlis-cli/src/dispatch.rs:1713`, which answers only
  inside a chat the app started) still says that its queue is waiting, in words, with no path, and
  lists no other chat's message and nothing of `refused/` (C-F2).
- **A message whose MAC does not match is an integrity failure, not a version.** It is still
  never delivered, and it stops being silent: the doctor says so, where today only a log line
  does (R26). It is moved aside, not removed (Q15), into a `refused/` folder
  beside the chats' folders. `refused/` is hardened (B6, D11):
  - it is made `0700`, and it is denied to every chat's sandbox. Before any move it is checked
    without following a link: a real folder, owned by this user, mode `0700`. Otherwise nothing
    is moved and the doctor says so (B-F15);
  - a message is moved into it as `<chat>-<seq>.json`, with no link followed and no file
    replaced; when that name is taken, a counter suffix is added (B-F9, round 3);
  - it holds at most 64 messages. Past that, a failing message stays where it is, undelivered,
    and is listed; it does not hold the drain, and it counts against no cap (B-F9, D-F5);
  - it is listed only as a count, each message's seq and its path, never its body;
  - nothing reads it as a message again. Its name is not a chat's number, so `chats_in`
    (`dispatchqueue.rs:677`) already passes it by, and a test holds that;
  - nothing removes it but the person. It has its own entry and tier in `docs/plane-format.md`
    (A3).
- **A key that is unavailable is not a MAC failure** (D4). When the spool's `keys.json` is
  refused, unreadable or damaged, no MAC can be checked, so the messages wait where they are. A
  message moves to `refused/` only when its key was read and its MAC still does not match.
- **Keys that read but hold no key for a message are not tampering either** (R17). That is what
  follows when a chat ends and its key is dropped, and every time `keys.json` is started fresh.
  Such a message is never moved to `refused/`: it is reported as undelivered, as an ended chat's
  queue is today, and kept on disk. The key read therefore has three answers, not a yes or no
  (`hookwire/spool.rs:1046`, `checks_as`, answers one boolean today). Today's report of an ended
  chat's queue copies only text that checked (`dispatchqueue/undelivered.rs:9`), so "reported"
  here is a doctor line by chat, seq and path, never the message's text and never an entry in an
  `undelivered/` report (A-F4, D-F5, round 3). Such a message can never be delivered, so it does
  not hold the drain and the floor may pass it; it counts against no cap (`MOST_QUEUED` counts the
  messages this build can deliver), and only the person removes it.

**`forge-accounts.json` stops downgrading on write.** It becomes a whole-file store at version
1, so a version-2 file is refused and kept.

**Only a store declared rebuildable may be rebuilt over a newer one** (R9). Rebuilding over a
newer record is a flag of its own in the declaration, set only where nothing is lost because the
store is derived. A transient store is not rebuildable: the forge budget, a browser's timeline
and the Inbox's updates would lose what they hold, and two builds on one clone would keep
rebuilding over each other. Transient stores, and the spool's `keys.json`, which holds the
replay floors, follow R2 in full. This is the one narrow exception to R2, beside R32's for the
event log and R36's below.

**A store's own removal keeps what it cannot read** (R36). A collector, or a store's own lifecycle
code, removes a whole record through the store's `remove`, which removes it only when it is
Understood; a collector keeps a record it cannot read, and the doctor counts it. Only a store
declared transient or rebuildable may remove a whole record whatever its version, as R9 and R32
allow: a resume file's `forget`, the browser timelines' cap, the spend files' retention and
`record_limit`. Dispatch records are neither, so their retention keeps a record it cannot read; a
store that is transient but a deny (the level records) or that holds a step cut short (the rename
and move journals) keeps the stricter rule (C-F5, A-F6, round 4).

### 3. Which private writers survive

- **`rewrite::replace` survives** as the one mechanism. It also serves the Plane-tier writers
  in its other modes, which this record does not touch. Three writers some stores use today stay,
  each inside the store as the store's declared write kind: `rewrite::replace` in secret mode, the
  same-user folder handle (`purlis_same_user::PrivateDirectory`) and the pinned write
  (`proposed::write_pinned`) (B-F3, C-F4, A-F4, round 4).
- **`machine::write_beside` and `plane::write_private` go.** Each store's write becomes the
  record store's write. A caller that writes a private file that is not a record calls
  `rewrite::replace` directly with `Mode::Private`. The two wrappers, and `glrefresh.rs:524`'s
  wrapper of the second, are removed once their last caller moves.
- **The one writer that bypasses all three moves onto it too:** the forge's
  tempfile-and-persist (`forge/etag.rs:104`, `write_private`, which the forge budget also calls,
  `forge/budget.rs:396`). It moves with the forge's stores, so the store is never bypassed
  meanwhile. The design note lists every store and its writer.
- The doctor's copy of the spool `keys.json` check goes: the doctor asks the store's `read`.

### 4. One approval ledger for code that runs

**An approval of code that runs is one interface, the approval ledger, over its adapters.** This
is B. "Ledger" is the code's word, never the window's. The ledger is built on the record store,
so every adapter's record gets R2's protection. The ledger decides the standing and the
lifecycle. Each adapter decides only its key, what it fingerprints, where its record is kept,
and what the person is shown.

**The standing is one type:**

- **Approved:** this subject, as it is now, was approved here.
- **New:** never approved here.
- **Changed**, with **why**: its **code** changed (a fingerprint), its **pin** changed (a page's
  folder, command or working folder, with who changed it), or its **reach** changed (§6).
- **Not asked**, with **why**: a script-free page, a platform where script never runs, a
  themes-only extension, or a built-in. Nothing runs here, so there is nothing to approve.

The words the CLI and the window already spell keep their spelling: `approved`, `new`,
`changed`, `pin-changed`, and for Not asked, `script-free`, `not-here` and the extension's
`themes`. `reach-changed` is new. `as-of` is not a standing: it is the window's word for a
version of a page shown from history, whose script is never run and never asks (E7). Not asked
does not satisfy every gate: a page that is `not-here` cannot be approved here, so its
Regenerate does not run (§8, Q4).

**The interface:**

- **`standing(subject, as presented now)`**, which reads only and never writes;
- **`approve(subject, what the person was shown)`**, which takes the fingerprints handed back
  from what the person was shown and never computes them again (charter-app#123's rule, kept).
  It replaces the subject's entry whole, so a key this build does not know never survives on an
  entry the person approves: such a key could be a grant to a newer purlis that nobody was shown
  (B4). Unknown keys are kept on every other entry and at the top level;
- **`forget(subject)`**, which says whether there was an approval;
- **`moved(from, to)`**, which carries the approvals of a place that moved, unknown keys and
  all, and answers how many.

**Only the window's press, or the person's own terminal where one exists today, calls
`approve`.** A chat never does. That is unchanged, and a guard test now lists every caller of
every function that writes an approval or a grant (the ledger's `approve`, each adapter's own
approve and record write, which stay public because the adapters wrap them, and `grant_plane`)
and of the store's Set it aside, and fails on a new one (B-F7, C23; B-F4, round 3).

**Four adapters keep six records** (A-F14, C22). The adapters, and the word each puts in the
Notice's cause key, are `extension`, `page` (one subject of two parts, the script and the
Regenerate code), `harness` (the declaration with its local profile) and `mcp`. Q16 calls MCP
the fifth because it counts the page and its Regenerate apart.

**Each adapter wraps its record; it does not take it over.** The extension record is a registry
as well as an approval: its source, whether it is on, whether it reads prompts, and its
unapproved rows, and its forget uninstalls (C7). The extension adapter answers standing and
lifecycle through the registry's own calls, and the registry keeps owning its writes.

### 5. The adapters

| Adapter | Key | Fingerprint | Record and tier | Approved by | Forgotten by | Moved by |
| --- | --- | --- | --- | --- | --- | --- |
| **Extension** | its id, and its folder in the entry | every path and byte of the folder, plus the manifest's protocol when it has a program | `<config>/extensions.json`, Machine, device-bound | the Extensions view's press (`app/src-tauri/src/extensions.rs:1530`, `approve_extension`, which calls `extension::approve` at `:1537`) | Remove (`app/src-tauri/src/extensions.rs:1551`, `forget_extension`) | carried when its fingerprint is unchanged, no link in the folder resolves outside it, and its state folder at the new place is absent or empty; otherwise it is a new subject and asks (Q13, B2) |
| **Page and Regenerate**, one subject | `artifact::approval::key`: the workspace's real path and the folder | two parts: the template (every path and byte but its data) with its reach (§6); and the Regenerate code folder with its command words, its path framed relative to its workspace (R8), stamped with the template's fingerprint (B1) | both files as they are, `<config>/artifact-approvals.json` and `<config>/artifact-regenerate-approvals.json`, Machine, device-bound | the page bar's press (`app/src-tauri/src/artifacts.rs:1400`, `artifact_approve`) and Regenerate's (`app/src-tauri/src/regenerate.rs:168`, `artifact_regenerate_approve`), one press for both under Q5 | an unpin here (`artifact/mod.rs:428`, `unpin_here`), which forgets **both** parts; and a workspace's removal (Q11) | a workspace rename (`wscmd/rename.rs:1292`, `follow_in_approvals`), which carries **both** parts |
| **Harness declaration** | the declaration's name | its whole digest | `.charter/harness-declarations-approved.json`, **Clone state**, as ADR 0069 has it | the new-chat picker's in-place "Approve and start" (ADR 0022), and the Notice's and the Blocking ask's Approve for a held start (`app/src-tauri/src/lib.rs:1865`, `approve_profile`; R34) | Forget on its harness card (R22). Removing it through purlis waits until a command that removes a declaration exists. A pull that removes it leaves the record, and the same bytes added back stand approved (Q9) | nothing to move: the record lives in the clone, and a renamed declaration is a new name |
| *(part of the harness-declaration adapter, Q10)* local profile | the profile's name | its kind, command and environment as declared | `.charter/harness-profiles-launched.json`, Clone state | the same press | as the declaration | as the declaration |
| **A persona's MCP servers** (Q16, R11) | the persona's name | the set of its approved servers' fingerprints | `.charter/mcp-approved.json`, Clone state | the person's own terminal (`purlis persona approve-mcp`, `personaverbs/approve.rs:110`), which replaces the persona's set | not raised: a decision, below | nothing to move: the record lives in the clone |

**Each adapter keeps its tier** (R3, ADR 0069). The ledger does not move a record between tiers,
and gap 12 is closed by naming the difference, not by removing it. A harness declaration and a
persona's MCP servers are committed to the project and approved per clone. An extension and a
page are about this machine's person, wherever the plane is.

**A Clone-state approval record that git tracks, or does not ignore, approves nothing** (B5,
R15). `.charter/harness-declarations-approved.json`, `.charter/harness-profiles-launched.json`
and `.charter/mcp-approved.json` are declared "must not be tracked". A record committed to the
project would approve code for everyone who clones it. The read refuses such a record: it reads
as nothing approved. Approve refuses while the record is tracked or not ignored, since approving
again would write the same file. The sentence says what is true: "purlis ignores {what} in this
project, because git could share them. Stop git tracking them, then approve again.", where
`{what}` is the record's own name: "your harness approvals", "your approvals of this machine's
profiles" and "your MCP server approvals" (E46, round 3). The path and the cause go in the
detail, as "git tracks this file" or "git does not ignore this file". Each cause draws the
remedies that cure it (E46): a record git does not ignore offers a press labelled "Ignore them in
git", which adds the line where `purlis doctor --fix local-ignore` adds it (`profiles.rs:1501`)
and says so in its detail; a tracked record draws only that press, and Copy command for
`git rm --cached <file>` once the record has been moved, when only the index holds it. **A record git brought is never read again as the
person's (R38).** Its cure, the "Ignore them in git" press and the doctor's fix alike, first moves the record into the held kept
folder (`<state folder>/app/kept/`, §1) as `<record>.git-brought-<UTC>`, with the store's no-replace
rename, then adds the ignore line; `git rm --cached` then only clears the index. The record reads
Absent, so every harness and MCP server in that project asks once, and "then approve again" is
true. The moved file is marked as brought by git and never offered back. A record someone untracked
by hand is still brought by git: one git no longer tracks whose content, hashed as git would store it
(`git hash-object --path=<path>`, so a committed line-ending rule cannot dodge the compare), equals
the blob the index or `HEAD` holds for its path approves nothing, and its cure moves it the same way, so `git rm --cached`
run first never makes the committed entries stand. Only a record that is not ignored, untracked and
matching no such blob keeps its entries, since a clone cannot bring an untracked file (B-F1, round
5; P1, regression check). The check uses `profiles`' hardened git runner (`profiles.rs:1588`,
`git_ignore_state`): if git cannot answer (missing, timed out, failed), nothing is approved, with
sentences of its own: the standing line is "purlis could not ask git whether it could share
{what}: {cause}. It ignores them until git answers.", and an Approve pressed meanwhile ends
"Nothing was approved. Try again." (D-F11, round 3; E65, round 4); outside a repository the
record passes. Each sentence is a line of `records-unread` and of the doctor.

**Every adapter has the same events** (R3): approved, changed, forgotten and moved. An event
the adapter does not raise is written down as a decision, not left as an omission. Each event
is raised in one place, which the design note lists, and each event tells every window, as an
approval of a page does today. The decisions:

- **Extension:** a move asks again, unless Q13's three conditions hold.
- **Harness declaration and local profile:** a move does not apply.
- **MCP servers** (R11): its record is `{persona: [fingerprint]}`, with no server name, so a
  changed server line cannot be told from a new one. So this adapter answers only **Approved**
  or **New**, its approve replaces the persona's whole set, as today, and it raises neither
  changed, forgotten nor moved. There is no format change. A server it does not stand approved
  for is withheld from the chat (`personaverbs/chatstart.rs:72`, in `servers`), which does not
  hold the chat. It lands after the four adapters R3 names (Q16). A bad file reads as empty
  there today, and the next approval writes only its own persona, losing every other approval.
  The record store's damage rule ends that.

**Removing a workspace** ends every pin in it. It also forgets their approvals, as unpinning
each would (Q11).

**What an approval of a page draws.** Every approval of a page part draws a new storage origin,
as ADR 0122 (amended 2026-11-10) has it, so the page starts with empty storage. That holds for an
approval asked again because the reach changed, since what the page may read is what the origin
protects (A6). Approving only the Regenerate part leaves the page's entry, and its origin, as
they are.

### 6. A page's reach is part of what is approved (R4, gap 17)

**The reach of a page is what its declared queries read, as purlis resolves them when it is
approved.** For each call the page declares (`<id>@<N>` with its parameter names;
`artifact/calls.rs`, `parse`), the reach holds one line:

- the id and the `@N` exactly as declared;
- the query's **scope**, `workspace` or `plane` (`catalogue.rs`, `Scope`);
- each **parameter the page passes that widens what it reads**, with the sentence that says what
  it then reads. For a core query this is `Query::widens` in `catalogue::CATALOGUE`. For an
  extension query it is the scope and `widens` the extension's manifest declares
  (`extension/queries.rs`, `Declared`), as the extension is approved now;
- for an extension query whose extension is not installed or not approved, `needs-extension`.

The reach is canonical JSON, sorted. The page's entry records it as `reach`, beside the
template's fingerprint: **the canonical lines themselves and their SHA-256** (R12). The digest
is the quick compare; the lines are what the Notice compares, so it can say what changed. The
baseline holds lines too, so an entry with no `reach` is compared the same way. A page whose own
declaration changes is already a changed template, because the declaration is in the
fingerprint. **The reach catches what changes outside the page:** a purlis release that moves a
query from `workspace` to `plane` or gives it a new widening parameter, and an extension update
that widens its query.

**A changed reach asks again.** At every load, and at every ask on the page's port, the standing
computes the reach afresh and compares the digest. A different digest is `Changed(reach)`: the
page renders without its script, its port answers `not-approved`, and the Notice (§7) compares
the lines now with the lines approved, line by line, and lists those that differ under two
headings, "Now also reads" and "No longer reads". A narrower reach also differs and asks too: one
press is a small price for a page that reads less. The reach travels with the standing the
window is served and is handed back on Approve, so nothing computes it at the press (B-F21).

**The plane grant is bound to the reach.** Today it is a flag on the entry
(`artifact/approval.rs:331`, `grant_plane`) that a new approval drops. It stays a flag, and it
is good only while the entry's `reach` is the reach now, so a query that newly reads the whole
plane cannot ride an old grant. On an entry with no `reach`, the grant holds exactly while the
entry passes the baseline. The grant press refuses unless the reach it was shown is the reach
now (B-F11, D-F16).

**What is not in the reach:**

- `since`, which is a release label and not a scope;
- `catalogue::API`, which the page declares in its own bytes;
- an extension capability's `since()`, which gates the manifest, not the page.

**Result shapes stay out of the reach** (Q7). For a core `@N` the shape is frozen by
`tests/fixtures/queries/v1.json`, so a wider result is a new `@N`, which the page must declare.
For an extension, a wider result is a change to the extension, and its own approval catches it.
A page approved while its extension was missing records `needs-extension` in its reach, so
installing the extension asks again, because what the page can read grew (Q8).

**Pages approved before the reach existed have no `reach`.** They are compared with a **baseline
reach** (Q6), which ships as data the running purlis reads, not as a test fixture (C8, D12): the
reach each core query had at the release that ships this. It is a checked-in file compiled into
the binary, never generated from the catalogue, and a test pins its SHA-256, so a query widened
on main later still asks and no change to the baseline goes unseen (D-F5, B-F12).
`tests/fixtures/queries/v1.json` holds no scope, so it cannot serve. The baseline is kept for as
long as an entry with no `reach` can exist, which is for good. Such a page asks again only if a
query it declares reads more now than it did in the baseline. A query the baseline does not hold
(one removed since, or one newer than it) asks. An extension's queries were never in any
baseline, so a page with no `reach` that declares one asks once. An older purlis that approves a
page writes its entry without `reach` (it replaces the entry whole), and the entry is then
compared with the baseline again: at worst one more ask, never a wider grant.

### 7. One re-ask, as one Notice (R4, gap 14)

**Every code approval asks in one place, except the new-chat picker, which approves a harness in
place (R34; E78, round 5): a `Notice`, the code-approval Notice**, following
ADR 0112's "one notice for one need". It is built from the `Notice` family
(`app/src/Notice.tsx`). Its cause key is `code-approval:<adapter>:<key>:<digest>`, where the
digest is of what the Notice shows (the fingerprints and the reach), so it settles when the
standing does, in every window, and a new change brings a dismissed Notice back (R20). The family
is in `KEPT` (`app/src/keptCauses.ts:19`), with the core as its source. It says:

- **what** asks: the name and where it is (an extension's folder, a page's folder and pin, a
  declaration's file);
- **what it may do:** a page's reach lines and whether it has Regenerate code; an extension's
  capabilities; a harness's command;
- **how it runs**, in the core's own words, carried to the window and rendered as given:
  an extension's `RUNS_AS_YOU` and `FINGERPRINTED` (`extension.rs:321`, `:339`), which ADR 0041
  requires and `app/src/Extensions.test.tsx:596` pins; for a harness, "A harness's command
  runs as you, with your files and your network." (E2); for Regenerate, today's words
  (`app/src/ArtifactRegenerate.tsx:151`); and a sentence each for a page's script and an MCP
  server (E19), each true in every case: "Its script runs in this window, sandboxed, and reads
  this artifact's own data and what is listed here." and "A server with a command runs as you
  whenever a chat of this persona starts, and each server is given the credentials shown." (E40,
  B-F13, round 3; a `url` server is one purlis does not run). A string a file controls, such as a
  name or a folder, is shown on one line (B-F18);
- **why it asks now:** "You have not approved it on this machine" (for the `harness` and `mcp`
  adapters, whose approvals are per clone, "You have not approved it for this copy of the
  project", E49), or, for a Regenerate entry that exists with no stamp or in the old framing,
  "purlis now records this approval with its artifact, so it asks once more." (E48), or what changed: its code
  ("Its template changed since you approved it." for a page, "It changed since you approved it."
  for an extension and for a harness, whose kind, command, environment or any byte of its
  declaration may be what changed (E76, round 5)), its pin and who
  changed it ("{who} changed its pin since you approved it.", from git history, with "Someone" when
  nobody is known, as today), or its reach, as "Now also reads" and "No longer reads" (E62, round
  4);
- **its way out**, as every Notice has: **Approve** (the `grants` button), and a second remedy
  of the adapter's own: "Review the folder" for a page, which opens its folder; "Open the
  extension", which opens its row in the Extensions view; "Show the declaration" for a harness,
  which opens its file (E62, round 4). An adapter
  with no window approve (MCP) offers Copy command instead, so Approve is optional (C14). Dismiss
  hides it until what it shows changes. For a page that already has a storage origin, the
  Notice says that approving starts it with empty storage (E20).

**Where it is drawn, for each adapter** (E3):

- **Extension:** in Notices, and on its row in the Extensions view, which keeps its standing
  phrase, with a press at its standing that opens the Notice again (R20; E67, round 4).
- **Page and Regenerate:** in the artifact tab's band of notices, and in Notices of its project.
  It is raised when the page is loaded or its Regenerate pressed, never by a scan of every pin
  (E27). The page bar keeps a short standing word for each standing, with a press at its standing
  that opens the Notice again (R20; E67, round 4). It keeps the sentences of `not-here` and `as-of`, which never ask
  (E12).
- **Harness declaration and local profile:** in Notices of its project, raised by a start the
  window made (the person's press), never by a refused `purlis dispatch` or by opening the project
  (B-F6, R27, round 3). While a chat is held on it, it is also a Blocking ask (below). The harness
  card's standing word reopens the subject's Notice, as the page bar's and the Extensions row's do
  (R27). The card that knows its subject is the new-chat picker's summary of the picked row, which
  shows "Approved", "Not approved" or "Changed since you approved it"; its "Review and approve…"
  raises the subject's Notice when none is up, since it is the person's press, and clears a kept
  dismissal when one is (E60, round 4). The picker is a modal dialog with the Notice behind it, so the
  press closes the picker and opens Notices on that Notice (E75, round 5).

  **Where a harness is approved today, and where it will be** (R34; E59, C-F1, round 4). The
  window approves a harness in four places (`approve_profile`). **The new-chat picker** keeps its
  in-place "Approve and start" (ADR 0022), because the person is already looking at the command;
  it reads the same ledger standing and shows the same lines and why line as the Notice, and it
  holds nothing. The other three are **held starts**, which raise the Blocking ask and the Notice,
  with Not now and its re-raise: the starts made without the picker, which `StartApproveAsk`
  serves today (a curation action, a theme chat, a file's "Start a chat here", the Skill studio's
  starts and an artifact discussion); **a waiting chat's "Review and approve…"**, on the Notice of
  a chat or task a launch could not start again and on its row in finished tasks, whose restart is
  a held start for that chat, and whose own approving question goes; and **the first-task tab's
  start**.
- **MCP servers:** in Notices of its project, raised when a chat of the persona that the window
  started withholds a server; each such start brings a dismissed one back, since it is the
  person's press, and a dispatched chat's start raises none (E44, B-F6, round 3). It settles on a
  re-read of the record at focus, since its approve happens in a terminal (C-F9). Its remedy is
  Copy command for `purlis persona approve-mcp`, the one approve this adapter has today, and it
  lists each credentialed server of the persona that is not approved, by its consent line, one
  line each (E40). See [New questions](#new-questions-for-the-operator-decided), N2.

**The standing word's press, in words** (E43, round 3). The standing stays shown text, in words a
person reads: for a page, "Script approved", "Script not approved", "Script changed", "Pin
changed", "What it reads changed" and "No script", with the `not-here` and `as-of` sentences as
they are; for an extension, its row's phrases as today. For a standing that asks (new, changed,
pin-changed, reach-changed), the press R20 and R27 put at the standing word is a button of the
families labelled "Review and approve…", which clears the kept dismissal, so the Notice is back in
the band and in Notices. A standing that does not ask has no Notice to open and no press.

**A code approval a chat is held on is a Blocking ask as well as the Notice** (R7, reverting
Q14). ADR 0121 decision 2 makes anything that stops one of the person's chats a Blocking ask,
and its amendments of 2026-11-10 ("consents and confirmations are asks") and 2026-11-12 ("a
persona's Allow is a Blocking ask") built that for two consents. A code approval follows them,
with no amendment of ADR 0121 needed:

- both are derived from one standing, so an answer in either place settles both;
- there is one ask per subject, of no one chat, however many chats wait on it. It is counted on
  the ✋, listed in Asks under its project, and notified, like every Blocking ask;
- a chat is **held** from a start the window made that was refused for this subject, until the
  subject is approved, the ask is answered Not now, the start is cancelled, or the project closes
  (R21). When the subject becomes Approved by any press (the picker's in-place approve, the Notice
  or the ask, in any window), every chat held on it starts through the full launch gate, as
  "Approve and start" starts them (E74, round 5);
- its answers are Approve, sent the fingerprints the row showed, and **Not now**, kept in memory
  while the project is open, as a persona's Allow has (R21). Not now puts away the ask for the
  starts already refused; a new start press for the subject clears Not now, is a new hold, and
  raises the ask again, and the press opens Asks on its row (R27, E37). The settle guard holds its
  Approve (ADR 0097). Its labels are written once in the asks registry, and the Notice draws them
  (E16);
- it is built on the asks of no chat that exist today, on the pattern of the profile sign-in ask
  (`app/src-tauri/src/asking.rs:3892`, `sign_in`) and the persona's Allow (`PersonaAllow`, with its
  Not now, `persona_allow_not_now` at `:5184`), not as a second model (C-F3, round 3);
- it is never offered a rule, and never answered by a rule, by Quiet, by a timeout or by a bulk
  triage action. It is answered only by the window's own command (the `local-ui` scope,
  `app/src-tauri/src/asking.rs:1257`, `the_window`), as the persona's Allow is; `purlis inbox`
  (the `approval` scope) may list it and never answers it, and no connection from a chat's process
  tree does (B-F8; B-F2, round 5);
- the Notice and the row show the profile's whole line, and Approve is refused when what would be
  shown now differs. "Approve and start" approves, then starts through the full launch gate, so a
  declaration changed in between starts nothing and asks again (B-F8);
- the start's press opens Asks on the subject's row, under Project, where an ask of no chat is
  listed, and no second Notice is raised for the refusal. The row says, under the profile's line,
  one line per held start: "{named}: starts with {typed} typed and not sent.", or for a restart,
  which types nothing, "{name}: starts again." Today's dialog clause about recording on this machine
  goes, since the why line says where the approval is kept (E18; E77, E81, round 5);
- **a refused `purlis dispatch` holds nothing and raises no ask, Notice or notification** (R27,
  E38). It exits 1 with today's sentence shape, pointing at the new-chat picker. **Its words are
  today's, unchanged (R37):** R27 recorded them as "…Start one chat on it from the new-chat picker,
  which asks in Asks (✋).", but R34 keeps the picker's in-place approve, so the picker asks in
  itself, and R37 drops that ending. The sentence stays today's: "persona '{persona}' names profile
  '{profile}', and what it runs has not been approved on this machine, so nothing was started.
  Start one chat on it from the new-chat picker, which shows its command and asks." ADR 0121
  decision 16's "waiting for the person" form is kept only for a start that really waits; a held
  start is refused and its press opens Asks, so none uses it today;
- dismissing the Notice never strands the chat, because the ask stays until it is answered.

Today only the harness-declaration adapter holds a chat. An extension, a page and an MCP server
asking hold none.

**A first ask and a re-ask are one Notice.** They differ in their "why" line, and only a re-ask
says what changed. Re-installing a changed extension therefore shows the re-ask, because the
ledger's standing chooses the sentence, not the way in. A first ask raised by the person's own
press (installing an extension, pressing Regenerate, starting a chat) uses the same Notice,
opened by the press (Q12), as ADR 0121 lets a Notice that answers a press open (amended
2026-11-12). **An Approve raised by a press finishes what the press began** (E6), and its label
says so, as today's labels do: "Approve and run" for Regenerate, "Approve and start" for a
harness, whose Notice and ask carry the start to make again once approved (C3). An extension's
Approve stays a separate press from its install and runs nothing. "Approve and run" is offered
only when the Notice was opened by a Regenerate press on a page that stands Approved (E36), and
only in that artifact tab's band, in the window where Regenerate was pressed, until the tab is
left or the Notice dismissed. In Notices and in every other window the same Notice is labelled
"Approve Regenerate code" and runs nothing (E45, round 3).

**The labels** (E16), a verb and what it acts on:

| When | Label |
| --- | --- |
| a page's script asks | "Approve script" |
| a Regenerate part asks, with no press behind it | "Approve Regenerate code" |
| opened by a Regenerate press | "Approve and run" |
| both parts ask | "Approve script and Regenerate code" |
| an extension asks | "Approve extension" |
| a harness asks, with no chat held | "Approve {profile}" |
| a harness asks, with one chat held | "Approve and start" |
| a harness asks, with several chats held | "Approve and start {n} chats" |

**One press for one subject.** The page and its Regenerate are one subject, so they ask in one
Notice, with a headed line for each part that asks: "Script", then "Regenerate code". A Notice
where one part asks has no heading (E53, round 3). One
Approve approves exactly the parts it lists, each by the fingerprint it showed, and its label
names them: "Approve script and Regenerate code" when both ask (Q5, E5).

**What it replaces:**

- the whole `ApproveExtension.tsx` dialog, first ask ("Trust it") and re-ask ("“X” has changed
  since you approved it", "Trust it anyway") alike, whose "What it declares" list becomes the
  Notice's "what it may do" (E15), and the comments that say "the next launch asks" where nothing
  does;
- the page bar's three re-ask sentences (`new`, `changed`, `pin-changed`) and its Approve script
  button (`ArtifactTab.tsx:236`, `ScriptStanding`, and `:264`, `standingSaid`);
- the Regenerate `AlertDialog` (`ArtifactRegenerate.tsx:144`) and `asksBecause`
  (`ArtifactRegenerate.tsx:165`). The Regenerate *failure* Notice stays: it is a refusal, not
  an approval;
- the re-ask in `StartApproval.tsx` for a project's harness declaration and a local profile;
- a waiting chat's own approving question, and the first-task tab's approve of the shown line
  (R34). The new-chat picker's in-place "Approve and start" stays (ADR 0022).

**What it does not replace:** the dispatch grant's Notice (`DispatchGrantNotice.tsx`). A
dispatch grant is about access (who may dispatch to whom), not code, and it stays outside the ledger
(§9). It is already a Notice, and the code-approval Notice follows its shape.

### 8. Regenerate runs only for an approved page (R3, gap 16)

**A Regenerate run is refused unless its page stands Approved**, checked in the same place and
at the same time as Regenerate's own standing: right before the run, in `run_pressed`
(`app/src-tauri/src/regenerate.rs:232`), and in `artifact_regenerate_about`
(`app/src-tauri/src/regenerate.rs:118`), so the button is disabled with its reason before any
press. `purlis artifact regenerate --check` prints both parts. All three call one core function,
page standing and Regenerate standing in, run or refusal out, so the rule is tested without a
session host and stays out of the Tauri crate (C11). The gate is an allow-list: it passes only
Approved and `script-free`, and refuses every other standing, one added later included. The
declaration it runs, the fingerprint it checks and the stamp it compares come from one read of
the template, so a template swapped between reads is never run (B-F9): the core function returns
what to run, the declaration it was given, and `run_pressed` runs only that value (B-F8, round
3). The refusal says "Approve its script first", since the window's noun is artifact, not page
(E25), and its remedy opens the subject's Notice. A `not-here` page is refused by Regenerate's own
platform check first (`artifact/regenerate.rs:74`), so it says the bar's `not-here` sentence,
never "Approve its script first", which nothing could do there (E55).

**A script-free page satisfies the gate**, because it has nothing to approve and its data is
inert. A page on a platform where script never runs does not, because it cannot be approved
there (Q4).

**The Regenerate part is stamped with the template it was approved beside** (B1). A pin's id is
drawn again from its folder, command and working folder (`artifact/pins.rs:107`, `id_of`), so a
re-pin with the same command has the same id. Without a stamp, a chat could re-pin a script-free
template under an orphaned Regenerate approval and revive it, and the template, which no approval
covers, names the data files Regenerate writes. So `approve` records on the Regenerate entry the
template's fingerprint it was joined with, script-free templates included. An entry with no
stamp, or a stamp that differs from the template now, reads as New. Every Regenerate approval
made before this asks once. The stamp is the template's fingerprint as the window was served it,
handed back with the parts on Approve, script-free pages included, and nothing computes it at the
press; a template changed between the Notice and the press refuses the press (B-F3, round 3).

**The rule for a lone part, said once** (B-F2, round 3). A script-free page never has a page part
(`artifact/approval.rs:265`), so "no page part" is not an orphan for it. A Regenerate part reads
New when its stamp is missing or is not the template's fingerprint now; "lone" means a Regenerate
part with no page part **while the template has script**. Before the stamp exists (ticket 5b), a
script-free page's Regenerate part is read by its own fingerprint, as today.

**The Regenerate fingerprint frames its code folder relative to the workspace** (R8). A code
folder is below a workspace's root, or a skill's own folder
(`artifact/regenerate.rs:245`, `allowed`). The first is framed by its path inside its
workspace, the second by its path inside the plane, which no workspace rename moves. A rename
then carries the approval as it is, and nothing is computed again. The fingerprint's framing
version goes up, so every Regenerate approval made before this asks once after the upgrade, in
the same press as B1's.

### 9. What stays out

- **Link hosts** (`<config>/artifact-link-hosts.json`) and **dispatch grants** (this
  machine's sandbox grants, `<config home>/purlis/sandbox-grants/<clone>.json`; the committed
  `[dispatch.grants]`; and `.charter/app/dispatch-never.json`) are about access, not code: what
  a page may open, and who may dispatch to whom. The older `.charter/app/sandbox.json` is no
  longer read or written outside tests (`sandbox/local.rs:20`), though `docs/plane-format.md`
  still lists it. They stay outside the ledger, and gap 18 (dispatch grants protected only by
  the sandbox) stays open. Where they are private JSON records, they still move onto the record
  store for its versioned read, because that is A, not B.
- **A harness's own allow list** and `PRE_ALLOWED` (gap 13), and **one query answering as two
  callers** (gap 15), are not approvals of code purlis runs.
- **C**, a failure value with the live hub owning the deadline and backoff (gaps 20 to 28), and
  **D**, a contract register checked against release tags (gaps 3, 4, 6, 8 and 9), are later
  efforts (R1). This record leaves `@N` selecting no behaviour and `since` unchecked. It records
  `@N` in the reach so that D can give it meaning without asking again.
- **Hook lines and the session protocol** are wires, not records. Gap 7, the newer hook line
  spooled under `v: 1`, is closed only where the spool is a record: a spool line is a row and
  is placed, skipped and kept like any row. The live wire stays as it is.

### 10. The migration: two added keys, new folders, no other format change

- **Every existing file keeps its name, place, tier and shape.** The page record and the
  Regenerate record stay two files. The ledger joins them by the key they already share, so no
  page approval is lost: a page approved yesterday stands Approved tomorrow. Regenerate
  approvals ask once (B1, R8).
- **Two keys are added, additively:** `reach` on a page approval's entry (§6), and the template
  stamp on a Regenerate approval's entry (§8). `docs/plane-format.md` records each before the
  code writes it, as the repository's rules require. No `written_by` key is added (Q1).
- **Four folders are added:** the dispatch queue's `refused/` (§2); `<state folder>/app/kept/`,
  which holds the kept copies of rows 4, 5 and 6 (R25, §1) and a record R38's cure moved; and, by
  R35, `<state folder>/vaults/kept/` and `<config home>/purlisd/kept/`; each with its entry and tier
  in `docs/plane-format.md` (A-N5, round 5); and the kept copies (`<file>.unread*`, `<file>.set-aside-*`), which take
  their store's tier and are recorded there before any real store moves (A-F9, round 3).
- **The approval records' `version: 1` is read at last.** 1, or absent, is Understood. A higher
  version is refused and kept (R2). The two harness approval records have no version, and they
  read as unstamped version 1.
- **The page subject's two files change together, under one lock.** The ledger takes the lock
  once and writes each file with `update_held` (C5). The order is chosen so that a crash between
  the two writes always fails closed (D9):
  - **Approve** writes the Regenerate part first and the page part last. A crash between them
    leaves a Regenerate part beside a page that does not stand Approved, which the gate (§8)
    refuses to run. The page asks, and approving it is the one more press.
  - **Forget** removes the page part first. A crash leaves a Regenerate part with no page part,
    which the gate refuses to run. Unpin and a workspace's removal forget first and remove
    second, and a forget that fails refuses the removal, so a pin made again with the same id
    never inherits an approval (D-F15).
  - **Moved** writes the Regenerate part first. A crash leaves the page part under the old key,
    and the page asks once at its new place. Nothing retries the move. The old entry names a
    place no workspace has now. A workspace made again at that place, with the same folder, pin
    and template, would find it standing Approved, for exactly the bytes approved.
- **A Regenerate part with no page part while its template has script, one under another pin, or
  one stamped with another template reads as New** (§8's rule for a lone part). Today's records hold such entries, left by every unpin before this
  shipped (gap 11). Reading them as New closes the gap without a sweep, because a read never
  writes. A re-pinned page's Regenerate therefore asks once more.
- **A malformed Regenerate entry is dropped from the read**, as a page entry already is
  (`artifact/approval.rs:233`, `approval_of`). It stays on disk, carried through like any entry
  the read does not use, and is replaced only when that subject is approved again.
- **Approvals lost to past renames are not recovered.** Nothing on disk says where they were.

## What it costs

- **Every store's read moves to one place.** About forty call sites change, one store per
  ticket, each behind a contract test that pins today's answer first (design note). Most stores
  are rewrites, not switches: their lock, root, serializer and unknown keys differ, and the
  declaration has to hold each.
- **An older purlis meets two new keys.** Each store's current reader must tolerate an unknown
  key before a key is written. A contract test holds that for each store, in the ticket that
  moves it, and a store that refuses unknown keys today is changed first.
- **A person may be asked once more.** Every Regenerate approval asks once after the upgrade (B1,
  R8). A re-pinned page's Regenerate asks again, and a page whose reach changes asks again, and
  loses its stored data with its new origin. Each is the point of R3, R4 and ADR 0122.
- **A refused store degrades a feature until purlis is updated, or the record is set aside.**
  That is R2's price, and it is smaller than the record lost today. For a store where starting
  fresh grants more, a damaged file degrades it too, until the person sets it aside (R13).
- **A skipped dispatch message holds its chat's queue** until a newer purlis delivers it or the
  person removes it (R16). That is the price of never delivering a message twice.
- **This protects a downgrade only to a build that has the record store**, and only for stores
  already moved. Going back past it runs the old readers (E22).

## What this changes in other records

Each change is an amendment file of its own in the amended record's folder
(`docs/adr/<NNNN>/<yyyy-mm-dd>-<slug>.md`), written when this record is accepted:

- **ADR 0122:** §4, an approval covers the template **and its reach**, and the plane grant is
  bound to the reach. §6, Regenerate runs only for an approved page and shares its page's
  lifecycle: unpin forgets it, and a rename carries it. Its approval is stamped with its
  template, and its fingerprint frames its folder relative to its workspace. Its bar's re-ask is
  the code-approval Notice.
- **ADR 0069:** no tier moves. The record store's declarations name each store's tier, and the
  test that every store the plane format names has a tier
  (`crates/purlis-core/tests/it/every_store_the_plane_format_names_has_a_tier.rs`) is joined by
  one that every store the record store declares is in `docs/plane-format.md`.
- **ADR 0112:** its "one notice for one need" now covers code approvals too: one Notice per
  subject, first ask and re-ask alike.
- **ADR 0053 and ADR 0073 §5:** an extension's and a harness declaration's approvals are adapters
  of the ledger. What they fingerprint is unchanged. An extension moved with its fingerprint
  unchanged keeps its approval under Q13's conditions.
- **ADR 0034:** the machine store's rule that a store it could not read is never overwritten
  becomes every private store's, through the record store, and its rule for a store that reads
  but does not parse (copy aside, then start fresh) becomes every whole-file store's, with no
  copy ever replaced.
- **ADR 0071:** a window stopped in memory that finds the marker gone reads the journal's end:
  it follows a re-arm this build can read, stays halted without a tamper line on a line it
  cannot read, and treats anything else as tampering, as today. A re-arm is still the window's
  alone; another window following it in the journal is that re-arm reaching every window (R28).
  It follows only a re-arm written after its own stop, and it states the residual it moves: today
  a running window puts the marker back whatever the journal says; after R28, a process running as
  the person that outlives the stop can re-arm a running window with one journal line, which the
  sandbox holds for a sandboxed chat only (B-F6, D-F2, round 4).
- ADR 0022 is not amended: the new-chat picker keeps its in-place approve (R34).
- ADR 0067 is not amended. R14's name-prefix denial is dropped (R25): the kept copies of the
  sandbox-held records go into `<state folder>/app/kept/`, inside a folder class 2 already denies
  whole, and the same folder under `$PURLIS_HOME` is denied beside the MCP record's own denial.

It leans on **ADR 0121** (asks, and Notices that open only on a press), **ADR 0104** (the UI
families), **ADR 0037** (`Notice` and its required way out), **ADR 0041** (an extension's honest
prompt) and **ADR 0120** (a refusal exits 1). It does not touch **ADR 0066**: an approval is
device-bound by its tier, not keyed by the device id.

## Considered options

- **A version check per store, fixed where it is wrong.** This is the cheapest. It is rejected
  because it is what produced five answers, and the next store would add a sixth.
- **Move every code approval to one record and one tier.** This would make gap 12 vanish on
  paper. It is rejected by R3 and ADR 0069: a harness declaration is the project's and is
  approved per clone, and an extension is this machine's person's.
- **Merge the page and Regenerate records into one file.** This is a format change and a
  migration that could lose approvals. The shared key already makes them one subject, so it is
  not needed.
- **Fingerprint the whole catalogue into every page approval.** This is simpler than a reach.
  It is rejected because every purlis release would ask every page again, and a prompt clicked
  by reflex is worse than none (ADR 0041, ADR 0122).
- **Keep a dialog for the re-ask and make it consistent.** It is rejected by ADR 0112 and ADR
  0121: a modal re-ask covers the window and the place it would be answered from.
- **Put link hosts and dispatch grants in the ledger.** They are about access, not code:
  what a page may open, and who may dispatch to whom. Folding them in would give a ledger whose
  "Changed" means nothing for them.
- **Carry a Regenerate approval across a rename by computing its fingerprint again** (B3). It
  would approve bytes nobody was shown, against charter-app#123's rule. R8 frames the path
  relative to the workspace instead.
- **Give the MCP record a server key** (C6), so it could answer Changed. It is a format change,
  and R11 declined it.

## Decisions on Q1 to Q16, R6 to R11 and R12 to R39

The operator decided Q1 to Q16 on 2026-11-15, after the draft, and R12 to R24 on the same day,
after review round 2. R25 to R28 were decided on 2026-11-16, after review round 3, each as
recommended (their points were P14 to P17), and R29 to R33 on the same day, on the five points the
round-3 fold left open (P18 to P22), each as recommended. R34 to R36 were decided on the same day,
after review round 4, each as recommended (P23 to P25), and R37 on 2026-11-17, on the one point
the round-4 fold left open (P26), as recommended, and R38 and R39 on 2026-11-18, after review round
5 (P27 and P28). "Default" means the operator took
the draft's recommendation. "Changed" means the operator chose an alternative. "Reverted" and
"narrowed" mean a ruling of review round 1 changed it.

- **Q1, changed.** No `written_by` key. The declaration names the release that introduced each
  version it knows, for `docs/plane-format.md` and a TooOld refusal's detail. A TooNew refusal
  cannot name a newer release, so it says a newer purlis wrote the record (E1).
- **Q2, default; narrowed by R9.** Only a store declared rebuildable may be rebuilt over a newer
  one.
- **Q3, default; narrowed by R10.** The refused stores share one Notice, `records-unread`, plus a
  doctor line per store.
- **Q4, default.** A script-free page satisfies the Regenerate gate. A page where script never
  runs does not.
- **Q5, default.** One Notice and one Approve, which approves exactly the parts it lists.
- **Q6, default.** Approvals with no reach are compared with a baseline reach frozen at the
  release that ships this, shipped as runtime data.
- **Q7, default.** Result shapes stay out of the reach.
- **Q8, default.** Installing an extension a page waited on asks again.
- **Q9, default; narrowed by R22.** A harness declaration's approval is forgotten by Forget on
  its card. Removing it through purlis is deferred until such a command exists. A pull that
  removes it does not forget it.
- **Q10, default.** The local-profile record is part of the harness-declaration adapter.
- **Q11, default.** Removing a workspace forgets its pages' approvals.
- **Q12, default.** A first ask raised by the person's press uses the same Notice.
- **Q13, changed.** A moved extension carries its approval when its fingerprint is unchanged, no
  link in it resolves outside its folder, and its state folder at the new place is absent or
  empty (B2).
- **Q14, changed; reverted by R7.** See R7.
- **Q15, changed.** A queued dispatch message whose MAC does not match is moved aside into
  `refused/`, kept for inspection and never delivered, and said in `purlis dispatch list` and the
  doctor. A message whose key is unavailable waits instead (D4).
- **Q16, default; narrowed by R11.** `.charter/mcp-approved.json` is a fifth ledger adapter,
  landed after the four, answering only Approved or New.
- **R6, recovery.** A damaged whole-file record is copied aside automatically, and the store
  starts fresh. A newer record is refused, and beside Update purlis it gets a second remedy, Set
  it aside and start fresh, which renames it on the person's press. Nothing is ever deleted.
- **R7, Q14 reverted.** A code approval a chat is held on is a Blocking ask in Asks as well as
  the Notice, both derived from one standing, as ADR 0121 requires. No amendment of ADR 0121 is
  needed.
- **R8, the Regenerate fingerprint.** It frames the code's path relative to the workspace, so a
  rename carries it and nothing is computed again. Existing Regenerate approvals ask once after
  the upgrade.
- **R9, Q2 narrowed.** Only stores declared rebuildable may be rebuilt over a newer one.
  Transient stores, and the spool's `keys.json` (replay floors), follow R2 in full. Rebuilding
  over a newer record is a declaration flag of its own.
- **R10, Q3 narrowed.** After a downgrade, the refused stores share one Notice listing them all,
  plus the doctor.
- **R11, Q16 narrowed.** The MCP-server adapter answers only Approved or New, with an approve that
  replaces the persona's set. There is no format change.
- **R12, the reach's stored form.** A page's approval entry stores the canonical reach lines plus
  their digest. The digest is the quick compare, and the Notice compares the lines for "Now also
  reads" and "No longer reads". The baseline holds lines too, so an entry with no `reach` is
  compared the same way.
- **R13, stores where starting fresh grants more.** A declaration flag. For such a store, Damaged
  is refused like Unplaceable: the fail-closed answer holds and writes refuse. Set it aside is
  offered on the person's press, and its sentence names what is forgotten. It is set on
  `dispatch-never.json` and `runners-unsandboxed.json`, whose fail-closed answer becomes "every
  runner counts as tainted". Each store's ticket checks whether the extension record's off set
  and `runners-stopped.json` have the same property. Test 25 covers these stores.
- **R14, files beside the sandbox-held records; narrowed by R25.** ADR 0067 is amended so the sandbox denies every
  sibling whose name starts with a held record's name, under every spelling of the state folder
  and under `$PURLIS_HOME`. The copy aside is an exclusive, no-follow `0600` create through the
  gated walk, with a counter suffix when the name is taken. R25 drops the prefix denial and the
  amendment of ADR 0067.
- **R15, a tracked approval record; narrowed by R38.** Approve refuses while the record is tracked or not ignored.
  The sentence says git could share the approvals, so stop git tracking them, then approve again,
  with the path and the cause in the detail. The window adds the ignore line, or offers Copy
  command. If git cannot answer, nothing is approved; outside a repository the record passes. The
  check uses `profiles`' hardened git runner. The sentence is a line of `records-unread` and of
  the doctor.
- **R16, the delivered floor.** A chat's drain stops at the lowest skipped message, as it stops
  at a `.part`. Later messages wait and are listed. The person can remove a skipped message to
  unblock the queue. There is no format change, and a copy of a delivered message is never
  delivered twice.
- **R17, keys readable but no key for a message.** Never moved to `refused/`. It is reported as
  undelivered, as an ended chat's queue is today, and kept on disk.
- **R18, the kill switch.** A last line this build cannot read counts as halted, and the window
  says why. This build's own re-arm, written after it, ends the halt. A stop or a re-arm is never
  refused for the bound or the 1000-line cap. An Unreadable journal with the marker present counts
  as halted. One build's re-arm is never read as tampering by another; R28 builds that rule
  in the window.
- **R19, the forge budget.** A budget this build cannot read counts as spent for background
  requests, never for a request a person waits on. It is a lockout, and its lock name is kept.
- **R20, a dismissed code-approval Notice.** The cause key carries a digest of what the Notice
  shows. The family joins `KEPT`. The page bar's and the Extensions row's standing words are
  presses that reopen the Notice. `records-unread` is keyed by the set of refused stores.
- **R21, Not now on the held-chat ask.** The Blocking ask gains Not now, kept in memory while the
  project is open. "Held" is defined, and the settle guard holds Approve (ADR 0097).
- **R22, forgetting a harness declaration.** A Forget press on the harness card. Removal through
  purlis is deferred until such a command exists, which narrows Q9.
- **R23, `runners-stopped.json`; joined by R29.** It stays out of the record store as the one named exception,
  because the host crate does not link the core. Its versioned read is fixed in place in the
  host.
- **R24, granularity.** The plan's oversized tickets are split, from 29 to about 45, and their
  edges redone (design note §4).
- **R25 (P14), copies of sandbox-held records; narrows R14; widened by R35.** The sandbox can deny only an exact
  path or a whole folder, and Claude Code's Linux sandbox holds no patterns. So for the
  sandbox-held records only (rows 4, 5 and 6), their copies aside and set-asides go into a folder
  the sandbox already denies whole (`.charter/app/kept/`), and their locks go into the session
  host's folder. Every other store keeps its copies beside the file (R6). The prefix-denial
  amendment of ADR 0067 is dropped. Test 50 asserts the denial is enforced by each backend's
  compiled profile (B-F1, C-F1, round 3).
- **R26 (P15), where skipped and refused dispatch messages are shown.** In `purlis doctor`: each
  skipped or refused message by chat, seq and path, what holds the queue, and the command to
  remove it. The `records-unread` Notice links to the doctor. The chat's own `purlis dispatch
  list` still says its queue is waiting (C-F2).
- **R27 (P16), the held-chat journey; narrowed by R37.** A new start press for a subject clears Not now and raises
  the ask again. The harness card's standing word reopens the subject's Notice, as R20 does for
  pages and extensions. A refused `purlis dispatch` holds nothing and raises no ask; it exits 1
  with today's sentence shape, pointing at the new-chat picker, which asks in Asks (✋). The
  "waiting for the person" form is kept only for a start that really waits (E37, E38).
- **R28 (P17), the kill switch with two windows.** A window stopped in memory that finds the
  marker gone follows a re-arm this build can read, if the journal ends on one (the journal is in
  a folder no chat opens); stays halted with R18's sentence and writes no tamper line, if the
  journal ends on a line this build cannot read; and treats anything else as tampering, as today.
  ADR 0071 gets an amendment. The change is in the window's `look`, tested at the window with two
  switches on one config home (D-F1).
- **R29 (P18), records the inventory never listed.** The core's records move in new tickets. The
  skill-level pair goes with row 19, and `unaccepted.json` is flagged starting fresh grants more.
  `hosts.jsonl` is fixed in place in the host and named as a second exception beside R23. The
  git-dir journals are out of scope (A-F1, C-F15).
- **R30 (P19), an ended chat with a kept message.** Its key and queue are kept, as for a chat
  whose messages could not be set aside. A reused chat number gets a fresh queue only once the old
  one is empty or removed (D-F3).
- **R31 (P20).** `waits.json`, `runners.json` and `runners-left.json` refuse a damaged file as R13
  does, with Set it aside on a press naming what is lost (D-F6, D-F7).
- **R32 (P21), event log retention.** Retention may delete a sealed segment whole, skipped rows
  included. This is a narrow exception to R2, like R9. Test 29 does not apply to the event log, for
  that reason (D-F8).
- **R33 (P22), the host's stop record while refused.** A host that starts stopped counts a runner
  answering "stopped already" as reached, so a re-arm reaches it. The core's doctor reads the
  record's version itself and prints a line with the remedy (D-F9, A-F5, C-F14, E49).
- **R34 (P23), where a harness is approved.** The new-chat picker keeps its in-place "Approve and
  start" (ADR 0022), because the person is already looking at the command. It reads the same
  ledger standing and shows the same lines and why. A waiting chat's "Review and approve…" restart
  and the first-task tab count as held starts: they raise the Blocking ask and the Notice, with Not
  now and its re-raise. Tickets 11c and 11c1 name all four places (E59, C-F1, round 4).
- **R35 (P24), R25 widened.** R25 applies to every store the sandbox holds by exact path, for
  reading or writing: their copies and set-asides go into the held folder. That covers
  `vault-links.json`, `peers.json`, `runners.json` and `runners-left.json` beside rows 4 to 6. Test
  50 lists them all (B-F2, round 4).
- **R36 (P25), collectors and removal by a store's own code.** A collector, or a store's own
  lifecycle code, may delete a whole record whatever its version only for a store declared
  transient or rebuildable, like R9 and R32. Every other store's collector keeps records it cannot
  read, and the doctor counts them (C-F5, A-F6, round 4).
- **R37 (P26), the refused dispatch's words; narrows R27.** A refused `purlis dispatch` on a
  profile nobody approved keeps today's sentence, unchanged: "persona '{persona}' names profile
  '{profile}', and what it runs has not been approved on this machine, so nothing was started.
  Start one chat on it from the new-chat picker, which shows its command and asks." R27's ending
  "which asks in Asks (✋)" is dropped, because R34 kept the picker's in-place approve. The CLI
  keeps "on this machine" on purpose. The same saying stays in its four places (the refusal and
  the persona view's line through `APPROVE_IT`, the wiring refusal, and the doctor's line), and
  the recorded scenarios `dispatch-on-a-profile-nobody-approved-is-refused` and
  `doctor-a-profile-nobody-approved-is-not-probed` do not move (E49, E61, round 4).
- **R38 (P27), R15's cure; narrows R15.** purlis's cure for a tracked approval record moves the
  record into the held `kept/` folder (R25, R35), marked as brought by git and never offered back.
  The record then starts empty, so every harness and MCP server in that project asks once. Test 38
  asserts each entry stands New afterwards. Story 36 holds (B-F1, round 5).
- **R39 (P28), the process.** Review round 5 is folded whole, every should-fix and nit included, as
  each review file proposes. One reviewer then checks only the folded fixes and the tickets they
  touch, for regressions. If that check is clean, building starts from the frontier (#6707, #6708).
  There is no full round 6.

## New questions for the operator (decided)

Two choices the rulings did not make surfaced while folding round 1 in. The operator decided
both on 2026-11-15 as recommended, so the record stands as written.

- **N1, decided: a refusal.** *Does an Unplaceable version get the damage rule or a refusal?* A version that is a
  string, a float, zero or negative is written by no purlis. The machine store today treats it as
  a file that does not parse, copies it aside and starts fresh. This record refuses it and offers
  Set it aside and start fresh on a press, because R6 names only garbage for the automatic rule.
  *Recommendation:* the refusal, as written: a version purlis cannot place is not proof that
  nothing newer wrote it.
- **N2, decided: Copy command.** *Does the MCP adapter's Notice get a window press?* Today a persona's MCP servers are
  approved only in the person's own terminal, so its Notice can offer only Copy command, which
  `app/src/Notice.guard.test.ts` lists as debt. A window press would be a new place that can
  approve code. *Recommendation:* Copy command, as written, and no new approve surface in this
  effort.
