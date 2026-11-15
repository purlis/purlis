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
Each is marked (Qn) or (Rn) where it applies and listed under
[Decisions](#decisions-on-q1-to-q16-and-r6-to-r11). The design note's appendix says where every
finding of round 1 went.

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
  and a migration for each version below its own that it reads (§2);
- its **bound** in bytes;
- its **lock** (below);
- its **serializer**, which stays what the file is written with today (Python's `indent=2`, with
  or without `ensure_ascii`, sorted keys, or serde's pretty print), so no file's bytes change
  when its store moves;
- whether it **keeps unknown keys**, and at which levels (about 45 of the 56 stores do not
  today);
- whether it **must not be tracked** by git (B5, §5);
- its **fail-closed answer** (§2) and its **busy sentence** (below).

**The read places the version as `compat::Schema` does, with two answers more.** The store
reads the document, or each row, and places its version key:

- **Understood:** absent (the store's unstamped version), or an integer from its oldest to its
  own. A version below its own is migrated on the read.
- **TooOld:** a positive integer below its oldest. It is its own placement, never Unplaceable
  (§2).
- **TooNew:** an integer above its own.
- **Unplaceable:** anything else: a string, a float, zero or a negative number. This is its own
  answer and its own sentence, never "says no version" (gap 2). Unlike `compat::schema`, a
  negative integer is not Understood.

Before it places the version, the read sorts out a file it cannot use:

- **Unreadable:** a link, a file over its bound, a file that is not a regular file, or one this
  process may not read. Nothing about the bytes is known, so the file is never touched.
- **Damaged:** bytes that are not JSON, or not the store's shape (a whole-file store whose
  document is not an object). A row store's line of that kind is a skipped row (§2).

Today's bounded reads keep their bounds; they move into the declaration.

**The interface has four calls:**

- **`read`** answers `Understood(value)`, `Absent`, or one of `TooOld`, `TooNew`, `Unplaceable`,
  `Unreadable` and `Damaged`, each with its sentence (§2).
- **`update`** is a locked read-modify-write. It holds the store's declared lock across the read
  and the write. It writes only over `Understood` or `Absent`, and over `Damaged` after the copy
  aside (§2). It never writes over any other answer. `update` never nests: a caller that must
  change two files under one lock takes the lock once and calls **`update_held`** for each, as
  `rewrite::update_held` does today.
- **Unknown keys are kept** where the store declares it. A value keeps every key this build does
  not know, at every level the store declares as open, and writes them back unchanged. A newer
  purlis that added a key without bumping the version loses nothing to an older one. An entry
  that `update` replaces on purpose is replaced whole, unknown keys and all (B4, §4).
- **The private write is one function**, `rewrite::replace` with `Mode::Private`, gated from
  the store's root. A store's own code never calls it.

**The lock is declared per store, never one lock for a tier** (A5, D8):

- a store in a folder no chat can open (the config home, the session host's folder) keeps a
  blocking lock. The machine store keeps `machine::Lock::named` and its lock file, so an older
  and a newer purlis still exclude each other;
- a store in a folder a chat can open (the plane's `.charter/`) takes a bounded lock, or a named
  lock in the session host's folder, as `rewrite::update_held`'s callers do;
- a store with a lock of its own today keeps it: the event log's lock, and the compare-and-swap
  of `skills.local.lock`;
- a store with no lock today gets one. An older purlis's write of it still takes none, so while
  an older and a newer purlis both run, a lost update stays possible. For an approval record
  that loss fails closed: a lost approval asks again;
- where a lock cannot be taken (a filesystem with no `flock`), the update goes ahead unlocked,
  as `machine::Lock` does today and for its reason: refusing would lose every write, not one.
- A bounded lock that is still held after its wait refuses with the store's **busy sentence**.
  `rewrite`'s own sentence ("a chat in this project may be holding it … close that chat") is
  right only for a folder a chat can open. An approval says "purlis is still recording another
  approval. Nothing was approved. Try again."

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
  {what}. A newer purlis wrote them, and this one is {version}. Nothing was changed. Update
  purlis." Its second remedy is **Set it aside and start fresh** (R6): on the person's press,
  purlis renames the file beside itself to `<file>.set-aside-<UTC time>`, never replacing a file
  already there, and the store starts empty. That is the way out after going from a development
  build back to the stable one, where Update purlis leads nowhere (D1). Nothing is deleted.
- **TooOld: migrated.** A record older than the oldest its store reads was written by an older
  purlis, so R2 does not hold it. Its store declares its migration. The default migration is the
  damage rule below: copy it aside, then start fresh. Its sentence: "purlis no longer reads {what}
  in the form an older purlis wrote them, so it set them aside and started fresh. Nothing was
  deleted." No store has a TooOld version on the day this ships (D3).
- **Unplaceable: refused, kept, and never written over.** "purlis cannot read {what}: their
  version is not one any purlis writes. Nothing was changed." Its one remedy is Set it aside and
  start fresh, on the person's press. Update purlis is not offered, because nothing says a newer
  purlis would read it. See [New questions](#new-questions-for-the-operator-decided), N1.
- **Damaged, whole file: copied aside automatically, then the store starts fresh** (R6). This is
  the machine store's rule of 2026-11-08, made every store's. The read answers `Damaged` and the
  feature reads nothing. The next `update` first copies the bytes to `<file>.unread`, or to
  `<file>.unread-<UTC time>` when one is already there, never replacing it, and then writes. Its
  sentence: "{what} were damaged, so purlis set them aside as {file name} and started fresh.
  Nothing was deleted." The machine store's `.unread` stops being one copy, the latest: R6 deletes
  nothing.
- **Unreadable: refused, and never touched.** "purlis cannot read {what}: {cause}. Nothing was
  changed. Make it a plain file purlis can read, or move it aside." These are the words the
  machine store already says for this case (`docs/plane-format.md`, ruled 2026-11-08).

**What the feature does meanwhile is the store's declared fail-closed answer**, which can only
ask more or do less, never grant more. An approval record that refuses reads as nothing
approved, so the subject asks, and approving refuses with the record's sentence. The design note
gives each store's answer in its own column, and names the stores whose answer locks the person
out of something: the approval records, `machine.json`, `profiles.json`, the spool's `keys.json`
and `secret-asks.json`. For those, Set it aside and start fresh is the remedy that matters.

**One Notice for every store that refuses** (R10). After a downgrade, many stores may refuse at
once, so they share one Notice, cause key `records-unread`, which lists each store with its
sentence and its own remedies, plus a doctor line per store. The Notice settles when the last
store reads again.

**A row store skips a row it cannot place, and keeps it on disk** (R2):

- For a JSONL file, a rewrite carries every skipped line through byte for byte, in its place.
  For a folder of files, the file is left.
- **A torn last line is a skipped row** (D7). An append always starts on a fresh line, so the
  row it writes never joins a torn one.
- **Pruning and the byte bound never drop a skipped row** (D6). A build that cannot read a row
  cannot know its age, so it keeps it. Skipped rows count toward the bound, and the build prunes
  its own rows first. If the skipped rows alone fill the bound, appending refuses with the
  store's sentence, and the doctor says why.
- The doctor counts each store's skipped rows.

**The dispatch queue stops deleting** (R2):

- A message of another `v` (`dispatchqueue.rs:625`) is left in place, and
  `purlis dispatch list` shows it as waiting for a newer purlis.
- **The delivered floor never passes a skipped message** (D5). Today a delivery raises the
  chat's floor (`dispatchqueue.rs:630`, against `delivered_floor`), and a message at or below it
  is then dropped as delivered (`hookwire/relay/carrying.rs:402`). Raising the floor stops below
  the lowest message this build skipped, so a newer purlis still finds it waiting.
- **A message whose MAC does not match is an integrity failure, not a version.** It is still
  never delivered, and it stops being silent: `purlis dispatch list` and the doctor say so, where
  today only a log line does. It is moved aside, not removed (Q15), into a `refused/` folder
  beside the chats' folders. `refused/` is hardened (B6, D11):
  - it is made `0700`, and it is denied to every chat's sandbox;
  - a message is moved into it with no link followed and no file replaced;
  - it holds at most 64 messages. Past that, a failing message stays where it is, undelivered,
    and is listed;
  - it is listed only as a count, each message's seq and its path, never its body;
  - nothing reads it as a message again. Its name is not a chat's number, so `chats_in`
    (`dispatchqueue.rs:677`) already passes it by, and a test holds that;
  - nothing removes it but the person. It has its own entry and tier in `docs/plane-format.md`
    (A3).
- **A key that is unavailable is not a MAC failure** (D4). When the spool's `keys.json` is
  refused, unreadable or damaged, no MAC can be checked, so the messages wait where they are. A
  message moves to `refused/` only when its key was read and its MAC still does not match.

**`forge-accounts.json` stops downgrading on write.** It becomes a whole-file store at version
1, so a version-2 file is refused and kept.

**Only a store declared rebuildable may be rebuilt over a newer one** (R9). Rebuilding over a
newer record is a flag of its own in the declaration, set only where nothing is lost because the
store is derived. A transient store is not rebuildable: the forge budget, a browser's timeline
and the Inbox's updates would lose what they hold, and two builds on one clone would keep
rebuilding over each other. Transient stores, and the spool's `keys.json`, which holds the
replay floors, follow R2 in full. This is the one narrow exception to R2.

### 3. Which private writers survive

- **`rewrite::replace` survives** as the one mechanism. It also serves the Plane-tier writers
  in its other modes, which this record does not touch.
- **`machine::write_beside` and `plane::write_private` go.** Each store's write becomes the
  record store's write. A caller that writes a private file that is not a record calls
  `rewrite::replace` directly with `Mode::Private`. The two wrappers, and `glrefresh.rs:524`'s
  wrapper of the second, are removed once their last caller moves.
- **The one writer that bypasses all three moves onto it too:** the forge's
  tempfile-and-persist (`forge/etag.rs:104`, `write_private`, which the forge budget also calls,
  `forge/budget.rs:396`). The design note lists every store and its writer.
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
`approve`.** A chat never does. That is unchanged.

**Each adapter wraps its record; it does not take it over.** The extension record is a registry
as well as an approval: its source, whether it is on, whether it reads prompts, and its
unapproved rows, and its forget uninstalls (C7). The extension adapter answers standing and
lifecycle through the registry's own calls, and the registry keeps owning its writes.

### 5. The adapters

| Adapter | Key | Fingerprint | Record and tier | Approved by | Forgotten by | Moved by |
| --- | --- | --- | --- | --- | --- | --- |
| **Extension** | its id, and its folder in the entry | every path and byte of the folder, plus the manifest's protocol when it has a program | `<config>/extensions.json`, Machine, device-bound | the Extensions view's press (`app/src-tauri/src/extensions.rs:1530`, `approve_extension`, which calls `extension::approve` at `:1537`) | Remove (`app/src-tauri/src/extensions.rs:1551`, `forget_extension`) | carried when its fingerprint is unchanged, no link in the folder resolves outside it, and its state folder at the new place is absent or empty; otherwise it is a new subject and asks (Q13, B2) |
| **Page and Regenerate**, one subject | `artifact::approval::key`: the workspace's real path and the folder | two parts: the template (every path and byte but its data) with its reach (§6); and the Regenerate code folder with its command words, its path framed relative to its workspace (R8), stamped with the template's fingerprint (B1) | both files as they are, `<config>/artifact-approvals.json` and `<config>/artifact-regenerate-approvals.json`, Machine, device-bound | the page bar's press (`app/src-tauri/src/artifacts.rs:1400`, `artifact_approve`) and Regenerate's (`app/src-tauri/src/regenerate.rs:168`, `artifact_regenerate_approve`), one press for both under Q5 | an unpin here (`artifact/mod.rs:428`, `unpin_here`), which forgets **both** parts; and a workspace's removal (Q11) | a workspace rename (`wscmd/rename.rs:1292`, `follow_in_approvals`), which carries **both** parts |
| **Harness declaration** | the declaration's name | its whole digest | `.charter/harness-declarations-approved.json`, **Clone state**, as ADR 0069 has it | the start's press (`app/src-tauri/src/lib.rs:1865`, `approve_profile`) | removing it through purlis, or Forget on its harness card. A pull that removes it leaves the record, and the same bytes added back stand approved (Q9) | nothing to move: the record lives in the clone, and a renamed declaration is a new name |
| *(part of the harness-declaration adapter, Q10)* local profile | the profile's name | its kind, command and environment as declared | `.charter/harness-profiles-launched.json`, Clone state | the same press | as the declaration | as the declaration |
| **A persona's MCP servers** (Q16, R11) | the persona's name | the set of its approved servers' fingerprints | `.charter/mcp-approved.json`, Clone state | the person's own terminal (`purlis persona approve-mcp`, `personaverbs/approve.rs:110`), which replaces the persona's set | not raised: a decision, below | nothing to move: the record lives in the clone |

**Each adapter keeps its tier** (R3, ADR 0069). The ledger does not move a record between tiers,
and gap 12 is closed by naming the difference, not by removing it. A harness declaration and a
persona's MCP servers are committed to the project and approved per clone. An extension and a
page are about this machine's person, wherever the plane is.

**A Clone-state approval record that git tracks, or does not ignore, approves nothing** (B5).
`.charter/harness-declarations-approved.json`, `.charter/harness-profiles-launched.json` and
`.charter/mcp-approved.json` are declared "must not be tracked". A record committed to the
project would approve code for everyone who clones it. The read refuses such a record: it reads
as nothing approved, with the sentence "purlis ignores the approvals in {file}, because git
tracks it. Approve again here."

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

The reach is canonical JSON, sorted, and its SHA-256 is recorded on the page's entry as
`reach`, beside the template's fingerprint. A page whose own declaration changes is already a
changed template, because the declaration is in the fingerprint. **The reach catches what
changes outside the page:** a purlis release that moves a query from `workspace` to `plane` or
gives it a new widening parameter, and an extension update that widens its query.

**A changed reach asks again.** At every load, and at every ask on the page's port, the standing
computes the reach afresh and compares the digest. A different digest is `Changed(reach)`: the
page renders without its script, its port answers `not-approved`, and the Notice (§7) lists the
lines under two headings, "Now also reads" and "No longer reads". A narrower reach also differs
and asks too: the digest cannot tell the direction, and one press is a small price for a page
that reads less.

**The plane grant is bound to the reach.** Today it is a flag on the entry
(`artifact/approval.rs:331`, `grant_plane`) that a new approval drops. It stays a flag, and it
is good only while the entry's `reach` is the reach now, so a query that newly reads the whole
plane cannot ride an old grant.

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
reach each core query had at the release that ships this. `tests/fixtures/queries/v1.json` holds
no scope, so it cannot serve. The baseline is kept for as long as an entry with no `reach` can
exist, which is for good. Such a page asks again only if a query it declares reads more now than
it did in the baseline. A query the baseline does not hold (one removed since, or one newer than
it) asks. An extension's queries were never in any baseline, so a page with no `reach` that
declares one asks once. An older purlis that approves a page writes its entry without `reach`
(it replaces the entry whole), and the entry is then compared with the baseline again: at worst
one more ask, never a wider grant.

### 7. One re-ask, as one Notice (R4, gap 14)

**Every code approval asks in one place: a `Notice`, the code-approval Notice**, following
ADR 0112's "one notice for one need". It is built from the `Notice` family
(`app/src/Notice.tsx`). Its cause key is `code-approval:<adapter>:<key>`, so it settles when the
standing does, in every window. It says:

- **what** asks: the name and where it is (an extension's folder, a page's folder and pin, a
  declaration's file);
- **what it may do:** a page's reach lines and whether it has Regenerate code; an extension's
  capabilities; a harness's command;
- **how it runs**, in the core's own words, carried to the window and rendered as given:
  an extension's `RUNS_AS_YOU` and `FINGERPRINTED` (`extension.rs:321`, `:339`), which ADR 0041
  requires and `app/src/Extensions.test.tsx:596` pins; and for a harness, "A harness's command
  runs as you, with your files and your network." (E2);
- **why it asks now:** "You have not approved it on this machine", or what changed: its code,
  its pin and who changed it (from git history, as today), or its reach, as "Now also reads" and
  "No longer reads";
- **its way out**, as every Notice has: **Approve** (the `grants` button), and a second remedy
  of the adapter's own: Review the folder, Open the extension, Show the declaration. Dismiss
  hides it until its cause changes.

**Where it is drawn, for each adapter** (E3):

- **Extension:** in Notices, and on its row in the Extensions view, which keeps a standing word.
- **Page and Regenerate:** in the artifact tab's band of notices, and in Notices of its project.
  The page bar keeps one short standing word, approved or not, which the frame's border already
  carries.
- **Harness declaration and local profile:** in Notices of its project. While a chat is held on
  it, it is also a Blocking ask (below).
- **MCP servers:** in Notices of its project. Its remedy is Copy command for
  `purlis persona approve-mcp`, the one approve this adapter has today. See
  [New questions](#new-questions-for-the-operator-decided), N2.

**A code approval a chat is held on is a Blocking ask as well as the Notice** (R7, reverting
Q14). ADR 0121 decision 2 makes anything that stops one of the person's chats a Blocking ask,
and its amendments of 2026-11-10 ("consents and confirmations are asks") and 2026-11-12 ("a
persona's Allow is a Blocking ask") built that for two consents. A code approval follows them,
with no amendment of ADR 0121 needed:

- both are derived from one standing, so an answer in either place settles both;
- there is one ask per subject, of no one chat, however many chats wait on it. It is counted on
  the ✋, listed in Asks under its project, and notified, like every Blocking ask;
- its answers are the Notice's, and its Approve is sent the fingerprints the row showed;
- a chat's start refusal names where it waits: its row in Asks, under the ✋;
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
Approve stays a separate press from its install and runs nothing.

**One press for one subject.** The page and its Regenerate are one subject, so they ask in one
Notice, with a headed line for each part that asks: "Script", then "Regenerate code". One
Approve approves exactly the parts it lists, each by the fingerprint it showed, and its label
names them: "Approve script and Regenerate code" when both ask (Q5, E5).

**What it replaces:**

- the re-ask branch of `ApproveExtension.tsx` ("“X” has changed since you approved it", "Trust
  it anyway"), and the comments that say "the next launch asks" where nothing does;
- the page bar's `<span>` and its Approve script button (`ArtifactTab.tsx:236`,
  `ScriptStanding`, and `:264`, `standingSaid`);
- the Regenerate `AlertDialog` (`ArtifactRegenerate.tsx:144`) and `asksBecause`
  (`ArtifactRegenerate.tsx:165`). The Regenerate *failure* Notice stays: it is a refusal, not
  an approval;
- the re-ask in `StartApproval.tsx` for a project's harness declaration and a local profile.

**What it does not replace:** the dispatch grant's Notice (`DispatchGrantNotice.tsx`). A
dispatch grant is about access (who may dispatch to whom), not code, and it stays outside the ledger
(§9). It is already a Notice, and the code-approval Notice follows its shape.

### 8. Regenerate runs only for an approved page (R3, gap 16)

**A Regenerate run is refused unless its page stands Approved**, checked in the same place and
at the same time as Regenerate's own standing: right before the run, in `run_pressed`
(`app/src-tauri/src/regenerate.rs:232`), and in `artifact_regenerate_about`
(`app/src-tauri/src/regenerate.rs:118`), so the button is disabled with its reason before any
press. `purlis artifact regenerate --check` prints both parts. The refusal says "Approve the
page first", and its remedy opens the subject's Notice.

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
made before this asks once.

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

### 10. The migration: two added keys, one new folder, no other format change

- **Every existing file keeps its name, place, tier and shape.** The page record and the
  Regenerate record stay two files. The ledger joins them by the key they already share, so no
  page approval is lost: a page approved yesterday stands Approved tomorrow. Regenerate
  approvals ask once (B1, R8).
- **Two keys are added, additively:** `reach` on a page approval's entry (§6), and the template
  stamp on a Regenerate approval's entry (§8). `docs/plane-format.md` records each before the
  code writes it, as the repository's rules require. No `written_by` key is added (Q1).
- **One folder is added:** the dispatch queue's `refused/` (§2), with its entry and tier in
  `docs/plane-format.md`, and the set-aside copies (`<file>.unread*`, `<file>.set-aside-*`),
  which take their store's tier.
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
    which the gate refuses to run.
  - **Moved** writes the Regenerate part first. A crash leaves the page part under the old key,
    and the page asks once at its new place. Nothing retries the move. The old entry names a
    place no workspace has now. A workspace made again at that place, with the same folder, pin
    and template, would find it standing Approved, for exactly the bytes approved.
- **A Regenerate part with no page part, one under another pin, or one stamped with another
  template reads as New.** Today's records hold such entries, left by every unpin before this
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
  That is R2's price, and it is smaller than the record lost today.

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

## Decisions on Q1 to Q16 and R6 to R11

The operator decided Q1 to Q16 on 2026-11-15, after the draft. "Default" means the operator took
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
- **Q9, default.** A harness declaration's approval is forgotten by removing it through purlis,
  or by Forget on its card. A pull that removes it does not forget it.
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
