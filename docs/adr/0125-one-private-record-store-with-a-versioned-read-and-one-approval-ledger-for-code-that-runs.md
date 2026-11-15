# One private record store with a versioned read, and one approval ledger for code that runs

**Proposed 2026-11-15**, for epic #6677. It follows the operator's rulings of 2026-11-15 on that
epic: **R1** (build A, the record store, with B, the approval ledger, on top of it; C and D are
later efforts), **R2** (a newer record is never deleted or overwritten), **R3** (one subject and
one lifecycle for a page and its Regenerate), **R4** (gaps 14 and 17 are in scope) and **R5**
(two review rounds before building). Those rulings are decided and this record does not reopen
them. The choices the rulings did not make were grilled with the operator on 2026-11-15 as Q1 to
Q16. Each is marked (Qn) where it applies and listed under [Decisions](#decisions-on-q1-to-q16).

The background is [`docs/surfaces.md`](../surfaces.md): gaps 1, 2, 7, 10, 11, 12, 14, 16 and 17,
and seams 1 and 2. The call-site inventory, the contract tests and the order of tickets are in
[`docs/design/record-store-and-approval-ledger.md`](../design/record-store-and-approval-ledger.md).
Every `path:line` below was read at `2dc0d73c2`. A line moves as the code changes, so the
function named beside each one is the anchor that lasts.

## Where purlis was

**Every private record reads its own version, its own way.** purlis keeps about forty private
JSON and JSONL records in the Machine and Clone-state tiers (ADR 0069). The core has 36 version
constants, and each store has its own hand-rolled bounded read. A record of a version the store
does not know gets one of five answers, depending on the store (gap 1):

- **Refused whole.** The extension record (`extension.rs:2473`, `read_record`) and the machine
  store (`machine.rs:2481`) refuse it, and neither will overwrite it (`extension.rs:2624`,
  `update`; `machine.rs:2561`, `update`).
- **Skipped and left on disk.** The dispatch record and the skill grades.
- **Deleted without a word.** A queued dispatch message of another `v` is removed and never
  delivered.
- **Downgraded on write.** `forge-accounts.json` reads its `version`, ignores it, and stamps `1`
  on the next write.
- **Written and never read.** `artifact-approvals.json` and `artifact-regenerate-approvals.json`
  write `"version": 1` (`artifact/approval.rs:432`, `change`) and never look at it again. The
  two harness consent records under `.charter/` carry no version at all
  (`profiletrust.rs:133`, `read_record`).

A garbage version also reads as a missing one (gap 2). Every one of these reads is a copy of
the same idea, and the doctor carries a further copy of the spool `keys.json` check
(`doctor/spool.rs` against `hookwire/spool.rs`).

**There is one placement already, for one file.** `compat::Schema` (`compat.rs:19`) sorts the
project file's `schema` into `Understood`, `TooNew` and `Unplaceable`, and `Schema::refusal`
(`compat.rs:31`) gives the one sentence `doctor`, the forge commands and `init` all say. It is
used for `charter.toml` alone.

**Private writes go through one mechanism under three names.** `machine::write_beside`
(`machine.rs:2717`) and `plane::write_private` (`plane.rs:466`) are each one line that calls
`rewrite::replace` (`rewrite.rs:554`) with `Mode::Private`. They differ only in the root the
walk is gated from. Only some read-modify-writes hold a lock: the machine store does
(`machine.rs:2561`), and the artifact approval record does not (`artifact/approval.rs:426`).

**Four approvals of code that runs, each on its own.** Each adapter has its own Standing or
Approval enum, key, record, validation and re-ask:

| Subject | Standing type | Record and tier | Forget | Carried on move |
| --- | --- | --- | --- | --- |
| Extension | `extension.rs:815` | `<config>/extensions.json`, Machine, device-bound | `extension.rs:2791` | no: a move is a new program (`extension.rs:2738`) |
| Page script | `artifact/approval.rs:59` | `<config>/artifact-approvals.json`, Machine, device-bound | `artifact/approval.rs:380`, called on unpin (`artifact/mod.rs:436`) | yes, on a workspace rename (`artifact/approval.rs:393`, called from `wscmd/rename.rs:1298`) |
| Regenerate code | `artifact/regenerate.rs:382` | `<config>/artifact-regenerate-approvals.json`, Machine, device-bound | none | no |
| Harness declaration | `profiletrust.rs:244` | `.charter/harness-declarations-approved.json`, Clone state | none | not applicable: the record is in the clone |

So the page and its Regenerate, which share one key (`artifact/approval.rs:106`, `key`, read
by `artifact/regenerate.rs:455`), part ways. Unpinning forgets the first and keeps the second,
and a rename carries the first and loses the second (gap 11). A Regenerate run checks only its
own standing, never the page's (`app/src-tauri/src/regenerate.rs:232`, `run_pressed`), so it
runs for a template nobody approved and writes what that template declares (gap 16). An
approval binds bytes, not what the declared queries reach, so a later purlis or extension that
widens a query is still covered by the old approval (gap 17). The re-ask is drawn five ways
(gap 14; the surfaces page counts four, and the harness declaration's start dialog is the
fifth):

- the extension dialog, whose re-ask is "Trust it anyway" (`app/src/ApproveExtension.tsx:59`);
- a sentence in a `<span>` in the page bar (`app/src/ArtifactTab.tsx:236`, `ScriptStanding`);
- the Regenerate `AlertDialog` (`app/src/ArtifactRegenerate.tsx:144`);
- the harness profile's start dialog (`app/src/StartApproval.tsx:78`, `StartApproveAsk`);
- and, for a dispatch grant, which is reach and not code, a `Notice`
  (`app/src/DispatchGrantNotice.tsx:169`).

## The decision

### 1. One record store for private records

**Every private JSON record purlis keeps in the Machine tier or in Clone state is read and
written through one module, the record store** (`purlis_core::record`; the name is the
build's to choose). It serves those two tiers and no others. A Plane file keeps
`compat::read`, because the project's schema governs it. A Keyring item is not a file.

**A store is declared, not hand-written.** Each store names, in one place:

- the file, as a path under its root (the config home for Machine, the plane for Clone state);
- its tier and marks, the same words as its `**Tier:**` line in `docs/plane-format.md`
  (ADR 0069);
- its shape: **whole file** (one JSON document, replaced whole) or **rows** (JSONL lines, or one
  file per record in a folder);
- the version it writes and the oldest it reads;
- its bound in bytes.

**The read is `compat::Schema`, made general.** The store reads the document, or each row, and
places its version field exactly as `compat::schema` places `charter.toml`'s:

- **Understood:** absent (the store's unstamped version), or an integer from its oldest to its
  own;
- **TooNew:** an integer above its own;
- **Unplaceable:** anything else, such as a string, a float or a negative number. This is its
  own answer and its own sentence, never "says no version" (gap 2).

Before it places the version, the read refuses, as one **Unreadable** answer with its cause, a
file that is a link, is over its bound, is not a regular file, or is not JSON. Today's bounded
reads keep their bounds; they move into the declaration.

**The interface has four calls:**

- **`read`** answers `Understood(value)`, `Absent`, or a **refusal**: `TooNew`, `Unplaceable`
  or `Unreadable`. A refusal carries one sentence in the shape of `Schema::refusal`: the store,
  the path, what was found, which purlis wrote it, and what to do, for example "Update purlis".
  The sentence is the same in the doctor, the CLI and the window.
- **`update`** is a locked read-modify-write. It holds `rewrite::Lock` on the store's folder
  across the read and the write, as `machine::update` holds its lock today. It refuses unless
  the read was `Understood` or `Absent`, and it never writes over a refusal.
- **Unknown keys are kept.** A value keeps every key this build does not know, at every level
  the store declares as open, and writes them back unchanged. A newer purlis that added a key
  without bumping the version loses nothing to an older one.
- **The private write is one function**, `rewrite::replace` with `Mode::Private`, gated from
  the store's root. It is never called by a store's own code.

**Which purlis wrote it.** No new key is added (Q1). A refusal names the record's version and
the store, "written by a newer purlis (record version N)", and its remedy is Update purlis. A
store that bumps its version records the release that introduced it in the store's
declaration, so the sentence can say "purlis X.Y or later" without anything on disk.

### 2. Whole-file and row stores under R2

R2 decides the behaviour. This record says where it lives:

- **A whole-file store that reads `TooNew` or `Unplaceable` refuses**, both to read and to
  write. The file stays as it is. What the feature then does is the store's declared
  **fail-closed** answer, which can only ask more or do less, never grant more. An approval
  record that refuses reads as nothing approved, so the subject asks, and approving refuses with
  the sentence. Where the refusal is shown (Q3): one Notice per refused store, cause `record-refused:<store>`, saying it was written by a newer purlis, with Update purlis as its fix, plus a doctor line.
- **A row store skips a row it cannot place and keeps it on disk.** For a JSONL file, a rewrite
  carries every skipped line through byte for byte, in its place. For a folder of files, the
  file is left. **The dispatch queue stops deleting**: a message of another `v`
  (`dispatchqueue.rs`) is left in place, and `purlis dispatch list` shows it as waiting for a
  newer purlis. A message whose MAC fails is an integrity failure, not a version, and it is
  still never delivered. It stops being silent: `purlis dispatch list` and the doctor say so,
  where today only a log line does. It is moved aside, not removed (Q15): into a `refused/` folder beside the queue, kept for the person to inspect, and never read as a message again. The doctor
  counts each store's skipped rows.
- **`forge-accounts.json` stops downgrading on write.** It becomes a whole-file store at version
  1, so a version-2 file is refused and kept.
- **A store marked rebuildable or transient** is read through the store, so it never misreads.
  It may be rebuilt over a newer one, because nothing is lost (Q2). This is the one narrow
  exception to R2, and it holds only for a store whose declaration says rebuildable or
  transient.

### 3. Which private writers survive

- **`rewrite::replace` survives** as the one mechanism. It also serves the Plane-tier writers
  in its other modes, which this record does not touch.
- **`machine::write_beside` and `plane::write_private` go.** Each store's write becomes the
  record store's write. A caller that writes a private file that is not a record calls
  `rewrite::replace` directly with `Mode::Private`. The two wrappers are removed once their
  last caller moves.
- **The writers that bypass it today move onto it too:** the forge's tempfile-and-persist
  (`forge/etag.rs`, which the forge budget also uses) and `glrefresh.rs:524`'s own private
  copy. The design note lists every store and its writer.
- The doctor's copy of the spool `keys.json` check goes: the doctor asks the store's `read`.

### 4. One approval ledger for code that runs

**An approval of code that runs is one interface, the approval ledger, over four adapters.**
The ledger is built on the record store, so every adapter's record gets R2's protection. The
ledger decides the standing and the lifecycle. Each adapter decides only its key, what it
fingerprints, where its record is kept, and what the person is shown.

**The standing is one type:**

- **Approved:** this subject, as it is now, was approved here.
- **New:** never approved here.
- **Changed**, with **why**: its **code** changed (a fingerprint), its **pin** changed (a page's
  folder, command or working folder, with who changed it), or its **reach** widened (§6).
- **Not asked**, with **why**: a script-free page, a platform where script never runs, a
  themes-only extension, a built-in. Nothing runs, so there is nothing to approve.

The words the CLI and the window already spell (`approved`, `new`, `changed`, `pin-changed`)
keep their spelling. `widened` is new.

**The interface:**

- **`standing(subject, as presented now)`**, which reads only and never writes;
- **`approve(subject, the fingerprint the person was shown)`**, which takes the fingerprint
  handed back from the drawing and never recomputes it (charter-app#123's rule, kept);
- **`forget(subject)`**, which says whether there was an approval;
- **`moved(from, to)`**, which carries the approvals of a place that moved and answers how many.

**Only the window's press, or the person's own terminal where one exists today, calls
`approve`.** A chat never does. That is unchanged.

### 5. The four adapters

| Adapter | Key | Fingerprint | Record and tier | Approved by | Forgotten by | Moved by |
| --- | --- | --- | --- | --- | --- | --- |
| **Extension** | its id, and its folder in the entry | every path and byte of the folder, plus the manifest's protocol when it has a program | `<config>/extensions.json`, Machine, device-bound | the Extensions view's press (`app/src-tauri/src/extensions.rs:1530`, `approve_extension`; `app/src-tauri/src/views.rs:760`, `:1235`) | Remove (`app/src-tauri/src/extensions.rs:1551`, `forget_extension`) | carried when its fingerprint is unchanged, since every path and byte is the same; otherwise it is a new subject and asks (Q13) |
| **Page and Regenerate**, one subject | `artifact::approval::key`: the workspace's real path and the folder | two parts: the template (every path and byte but its data) with its reach (§6), and the Regenerate code folder with its command words | both files as they are, `<config>/artifact-approvals.json` and `<config>/artifact-regenerate-approvals.json`, Machine, device-bound | the page bar's press (`app/src-tauri/src/artifacts.rs:1400`, `artifact_approve`) and Regenerate's (`app/src-tauri/src/regenerate.rs:168`, `artifact_regenerate_approve`) | an unpin here (`artifact/mod.rs:428`, `unpin_here`), which forgets **both** parts | a workspace rename (`wscmd/rename.rs:1292`, `follow_in_approvals`), which carries **both** parts |
| **Harness declaration** | the declaration's name | its whole digest | `.charter/harness-declarations-approved.json`, **Clone state**, as ADR 0069 has it | the start dialog's press (`app/src-tauri/src/lib.rs:1865`, `approve_profile`) | removing it through purlis, or Forget on its harness card. A pull that removes it leaves the record, and the same bytes added back stand approved (Q9) | nothing to move: the record lives in the clone, and a renamed declaration is a new name |
| *(part of the harness adapter, Q10)* local profile | the profile's name | its kind, command and environment as declared | `.charter/harness-profiles-launched.json`, Clone state | the same press | as the declaration | as the declaration |

**Each adapter keeps its tier** (R3, ADR 0069). The ledger does not move a record between tiers,
and gap 12 is closed by naming the difference, not by removing it. A harness declaration is
committed to the project and approved per clone. The other three are about this machine's
person, wherever the plane is.

**Every adapter has the same events** (R3): approved, changed, forgotten and moved. An event
the adapter does not raise is written down as a decision: "a move asks again", for example, not
an omission. Each event is raised in one place, which the design note lists, and each event
tells every window, as an approval of a page does today.

**A persona's MCP servers are approved too.** `.charter/mcp-approved.json`
(`personaverbs/mcp.rs:32`, `approve` at `:374`) holds the fingerprints of each persona's MCP
server lines that this clone approved. That is code that runs, and R3 does not name it. A bad
file reads as empty there, and the next approval then writes only its own persona, losing every
other approval. It moves onto the record store in any case. It also becomes a fifth adapter of
the ledger, in Clone state like the harness declaration, with the same events and the
code-approval Notice. It lands after the four adapters R3 names (Q16).

**Removing a workspace** ends every pin in it. It also forgets their approvals, as unpinning
each would (Q11).

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

**Widening asks again.** At every load, and at every ask on the page's port, the standing
computes the reach afresh and compares the digest. A different digest is `Changed(reach)`: the
page renders without its script, its port answers `not-approved`, and the drawing (§7) lists the
lines that are new. A narrower reach also differs and asks too: the digest cannot tell the
direction, and a drawing that says "reads less now" costs one press.

**The plane grant is bound to the reach.** Today it is a flag on the entry
(`artifact/approval.rs:331`, `grant_plane`) that a new approval drops. It stays a flag, and it
is good only while the entry's `reach` is the reach now, so a query that newly reads the whole
plane cannot ride an old grant.

**What is not in the reach:**

- `since`, which is a release label and not a scope;
- `catalogue::API`, which the page declares in its own bytes;
- an extension capability's `since()`, which gates the manifest, not the page.

A result's shape is left out too. For a core `@N` it is frozen by
`tests/fixtures/queries/v1.json`, so a wider result is a new `@N`, which the page must declare.
For an extension, a wider result is a change to the extension, and its own approval catches it.
Result shapes stay out of the reach (Q7). A page approved while its extension was missing records `needs-extension` in its reach, so installing the extension asks again, because what the page can read grew (Q8).

Pages approved before the reach existed have no `reach`. They are compared with a **baseline reach**: the reach the catalogue gave at the release that ships this, frozen in a fixture. Such a page asks again only if something widened since that release (Q6).

### 7. One re-ask drawing (R4, gap 14)

**Every code approval asks in one drawing: a `Notice`, the code-approval Notice**, following
ADR 0112's "one notice for one need". It is built from the `Notice` family
(`app/src/Notice.tsx`). Its cause key is `code-approval:<adapter>:<key>`, so it settles when the
standing does, in every window. It says:

- **what** asks: the name and where it is (an extension's folder, a page's folder and pin, a
  declaration's file);
- **what it may do:** a page's reach lines and whether it has Regenerate code, an extension's
  capabilities, a harness's command;
- **why it asks now:** "You have not approved it on this machine", or what changed: its code,
  its pin and who changed it (from git history, as today), or the reach lines that are new;
- **its way out**, as every Notice has: **Approve** (the `grants` button), and a second remedy
  of the adapter's own (Review the folder, Open the extension, Show the declaration). Dismiss
  hides it until its cause changes.

**What it replaces:**

- the re-ask branch of `ApproveExtension.tsx` ("“X” has changed since you approved it", "Trust
  it anyway"), and the comments that say "the next launch asks" where nothing does;
- the page bar's `<span>` and its Approve script button (`ArtifactTab.tsx:236`,
  `ScriptStanding`, and `:264`, `standingSaid`). The bar keeps one short state word, approved
  or not, which the frame's border already carries;
- the Regenerate `AlertDialog` (`ArtifactRegenerate.tsx:144`) and `asksBecause`
  (`ArtifactRegenerate.tsx:165`). The Regenerate *failure* Notice stays: it is a refusal, not
  an approval;
- the re-ask in `StartApproval.tsx` for a project's harness declaration and a local profile.

**What it does not replace:** the dispatch grant's Notice (`DispatchGrantNotice.tsx`). A
dispatch grant is reach between personas, not code, and it stays outside the ledger (§9). It is
already a Notice, and the code-approval Notice follows its shape.

**A first ask and a re-ask are one drawing.** They differ in their "why" line, and only a
re-ask says what changed. Re-installing a changed extension therefore shows the re-ask, because
the ledger's standing chooses the sentence, not the way in. A first ask raised by the person's
own press (installing an extension, pressing Regenerate) uses the same Notice, opened by the
press (Q12). A code approval that a waiting chat is held on is a Notice only. It is not also
listed in Asks, because the chat's start refusal already says why it waits (Q14).

**One press for one subject.** The page and its Regenerate are one subject, so they ask in one
Notice that lists each part that asks. One Approve approves exactly the parts it lists,
each by the fingerprint it showed (Q5).

### 8. Regenerate runs only for an approved page (R3, gap 16)

**A Regenerate run is refused unless its page stands Approved**, checked in the same place and
at the same time as Regenerate's own standing: right before the run, in `run_pressed`
(`app/src-tauri/src/regenerate.rs:232`), and in `artifact_regenerate_about`
(`app/src-tauri/src/regenerate.rs:118`), so the button is disabled with its reason before any
press. `purlis artifact regenerate --check` prints both parts. The refusal says "Approve the
page first", and its remedy opens the subject's Notice. A script-free page satisfies the gate,
because it has nothing to approve and its data is inert. A page on a platform where script never
runs does not, because it cannot be approved there (Q4).

### 9. What stays out

- **Link hosts** (`<config>/artifact-link-hosts.json`) and **dispatch grants** (this
  machine's sandbox grants, `<config home>/purlis/sandbox-grants/<clone>.json`; the committed
  `[dispatch.grants]`; and `.charter/app/dispatch-never.json`) are about reach, not code. The
  older `.charter/app/sandbox.json` is no longer read or written outside tests
  (`sandbox/local.rs:20`), though `docs/plane-format.md` still lists it. They stay outside the ledger, and gap 18 (dispatch grants protected only by the
  sandbox) stays open. Where they are private JSON records, they still move onto the record
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

### 10. The migration: no format change but two added keys

- **Every existing file keeps its name, place, tier and shape.** The page record and the
  Regenerate record stay two files. The ledger joins them by the key they already share, so no
  approval is lost: an entry approved yesterday stands Approved tomorrow.
- **One key is added, additively:** `reach` on a page approval's entry (§6).
  `docs/plane-format.md` records it before the code writes it, as the repository's rules
  require. No `written_by` key is added (Q1).
- **The approval records' `version: 1` is read at last.** 1, or absent, is Understood. A higher
  version is refused and kept (R2). The two harness consent records have no version, and they
  read as unstamped version 1.
- **The page subject's two files change together, under one lock.** The ledger holds
  `rewrite::Lock` on purlis's config folder across both. Forget removes the page part first.
  A crash between the two writes then leaves a Regenerate part with no page part, which the
  gate (§8) refuses to run, so a half-done forget fails closed. Moved writes the Regenerate part
  first. A crash then leaves the page asking at its new place: one more press, never a wider
  grant.
- **A Regenerate part with no page part, or one under another pin, reads as New.** Today's
  records hold such entries, left by every unpin before this shipped (gap 11). Reading them as
  New closes the gap without a sweep, because a read never writes. A re-pinned page's
  Regenerate therefore asks once more.
- **A malformed Regenerate entry is dropped from the read**, as a page entry already is
  (`artifact/approval.rs:233`, `approval_of`). It is kept on disk under R2's "unknown keys are
  kept", and replaced only when that key is approved again.
- **Approvals lost to past renames are not recovered.** Nothing on disk says where they were.

## What it costs

- **Every store's read moves to one place.** About forty call sites change, one store per
  ticket, each behind a contract test that pins today's answer first (design note).
- **An older purlis meets `reach`.** Each store's current reader must tolerate an unknown key
  before it ships. A contract test holds that for every store, and a
  store that refuses unknown keys today is changed first.
- **A person may be asked once more.** A re-pinned page's Regenerate asks again, and a page whose
  reach widens asks again. Both are the point of R3 and R4.
- **A refused store degrades a feature until purlis is updated.** That is R2's price, and it is
  smaller than the record lost today.

## What this changes in other records

Each change is an amendment file of its own in the amended record's folder
(`docs/adr/<NNNN>/<yyyy-mm-dd>-<slug>.md`), written when this record is accepted:

- **ADR 0122:** §4, an approval covers the template **and its reach**, and the plane grant is
  bound to the reach. §6, Regenerate runs only for an approved page and shares its page's
  lifecycle: unpin forgets it, and a rename carries it. Its bar's re-ask is the code-approval
  Notice.
- **ADR 0069:** no tier moves. The record store's declarations name each store's tier, and the
  test that every store the plane format names has a tier
  (`crates/purlis-core/tests/it/every_store_the_plane_format_names_has_a_tier.rs`) is joined by
  one that every store the record store declares is in `docs/plane-format.md`.
- **ADR 0112:** its "one notice for one need" now covers code approvals too: one Notice per
  subject, first ask and re-ask alike.
- **ADR 0053 and ADR 0073 §5:** an extension's and a harness declaration's approvals are adapters
  of the ledger. What they fingerprint is unchanged.
- **ADR 0034:** the machine store's rule that a store it could not read is never overwritten
  becomes every private store's, through the record store.

It leans on **ADR 0121** (asks, and Notices that open only on a press), **ADR 0104** (the UI
families) and **ADR 0037** (`Notice` and its required way out). It does not touch **ADR 0066**:
an approval is device-bound by its tier, not keyed by the device id.

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
- **Put link hosts and dispatch grants in the ledger.** They are reach between people, personas
  and hosts, not code. Folding them in would give a ledger whose "Changed" means nothing for them.

## Decisions on Q1 to Q16

The operator decided these on 2026-11-15, after the draft. "Default" means the operator took the
draft's recommendation. "Changed" means the operator chose an alternative.

- **Q1, changed.** No `written_by` key. A refusal names the record's version, and a store's
  declaration names the release that introduced each version.
- **Q2, default.** A rebuildable or transient store may be rebuilt over a newer one.
- **Q3, default.** One Notice per refused store, `record-refused:<store>`, plus a doctor line.
- **Q4, default.** A script-free page satisfies the Regenerate gate. A page where script never
  runs does not.
- **Q5, default.** One Notice and one Approve, which approves exactly the parts it lists.
- **Q6, default.** Approvals with no reach are compared with a baseline reach frozen at the
  release that ships this.
- **Q7, default.** Result shapes stay out of the reach.
- **Q8, default.** Installing an extension a page waited on asks again.
- **Q9, default.** A harness declaration's approval is forgotten by removing it through purlis,
  or by Forget on its card. A pull that removes it does not forget it.
- **Q10, default.** The local-profile record is part of the harness adapter.
- **Q11, default.** Removing a workspace forgets its pages' approvals.
- **Q12, default.** A first ask raised by the person's press uses the same Notice.
- **Q13, changed.** A moved extension carries its approval when its fingerprint is unchanged.
- **Q14, changed.** A code approval a waiting chat is held on is a Notice only, not also an Ask.
- **Q15, changed.** A queued dispatch message whose MAC fails is moved aside into `refused/`,
  kept for inspection and never delivered, and said in `purlis dispatch list` and the doctor.
- **Q16, default.** `.charter/mcp-approved.json` is a fifth ledger adapter, landed after the four.
