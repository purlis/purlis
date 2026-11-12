# 0124. Who judges a chat, and how much

Status: proposed (2026-11-12). The person's design, agreed over 143 decisions (Q1-Q143) in four
review rounds on 2026-11-12, carried here as its v3 states it. Amends ADRs 0067, 0069, 0076, 0090,
0093, 0096, 0112, 0119 and 0121 and its amendments, and reverses toolgate's "no switch" ruling
(see [What it amends](#what-it-amends)).

**The UI sections are provisional** (decisions 4, 5, 10, 11, 12, 13 and 14, and the words and
places named in the others). They wait on the person's review of the prototype on branch
`prototype-v2-who-judges-a-chat-from-desi-wc7ypwec`, and an amendment may change them after it.
What enforces, what is recorded and where it is kept are not provisional.

## Context

A chat is stopped by several judges: purlis's sandbox, purlis's guards, the harness's own check
(Claude Code's permission prompt and auto-mode classifier, opencode's permission asks, Codex's
approval stops), a policy, and the person. Before this record each judge stopped a chat in its
own place, and many of them stopped it where the person could not see it:

- **Black holes.** A stop the person cannot see in Asks (✋), or cannot answer from there. purlis
  registers no `PermissionDenied` hook, so a classifier refusal is heard by no one
  (`hookreg.rs`). A guard's deny is never reported to the app (`purlis-cli/src/guard.rs`). Codex
  rejects what it would ask (`sandbox/codex.rs`). No purlis hook sees a non-purlis MCP tool, so
  under any mode an Atlassian `executeDestructive` or an Artifact publish meets no purlis check.
  A `terminal` ask is answerable only in its pane.
- **Dead loops.** A stop that recurs with no way out: the same refusal again and again, an Allow
  that doesn't stick (#5242, #5244), a chat rewording a refused command, a fail-closed state
  (#6315: the classifier was unavailable, and every Bash call was refused for the rest of the
  session with nobody asked). `looping` counts only identical failures and the person's own
  refusals (ADR 0121, amended 2026-11-10); guard and classifier refusals are not counted.
- **The classifier's noise, measured.** Of 95 distinct classifier events in this workspace's
  records (2026-10-28 to 2026-11-12, medium confidence): **5** would have been stopped anyway by
  a layer that holds in every mode (3 by the sandbox or a deny rule, 2 by guard A4); **27** were
  correct stops only the classifier made, 23 of them `land.mjs` or an enqueue refused as "Merge
  Without Review"; **59** were false positives on the chat's own reversible work (18 `gh pr
  ready`, 28 forge text writes, 7 lease or force pushes of its own branch, 2 local commits, 3
  reads of public source, 1 label). 3 are ambiguous and 1 is #6315's no-verdict. No purlis layer
  would catch any of the 59 under `bypassPermissions`, and none sees `node tools/land.mjs`.
- **`bypassPermissions` is purlis's unattended signal.** Guard A4 switches on only under it
  (`floorguard.rs`), and dispatch, the briefing and the commit gate read it too
  (`dispatchunattended.rs`, `briefing.rs`, `commitgate.rs`). Codex reports it for every chat. So
  running the full sandbox in bypass, to drop the classifier's noise, would have changed what
  those read, and a mode is not a fact about whether anyone is watching.

The design was reviewed four times: for security; for the UX model, journeys, copy and
accessibility; for consistency, personas and buildability; and for setup, noise against
observability, a simple surface, a red team and governance. Each round revised the one before.
The security round found four blockers: bypass as the unattended signal, MCP servers outside the
sandbox, forge controls that only read words, and built-in tools no hook sees. This record
carries the last state only.

## The decision

### 1. Principles

1. **Protection when you need it** (Q113). purlis *can* make an agentic coding environment
   secure. Protection is chosen when a project is set up and is easy to change later. The goals,
   all at once: user friendly, secure, governed, controllable, observable, not noisy, and basic on
   the surface with depth underneath.
2. **The one-sentence model.** *purlis keeps each chat inside limits that you, your project or
   your organisation choose. When a chat reaches one, purlis asks you, or blocks it and tells
   you, and it always says what stopped it and what you can do.*
3. **The invariant** (Q1). Every stop of a chat shows up in exactly one place, says who stopped
   it, and offers a way out. No black holes, no dead loops.
4. **Harness-agnostic** (Q67, Q88). Every concept is purlis's, and each harness maps onto it
   through its adapter. Harness-specific names (`bypassPermissions`, `auto`, `PermissionDenied`)
   appear only in adapters and ADR notes. Where a harness can't report something, the UI says
   **Can't tell**. This record carries a matrix of layers by harness, with one test per cell
   ([Consequences](#consequences)).
5. **Honesty** (Q59, Q76, Q109, Q139, Q141). Nothing promises more than it enforces, and known
   gaps are stated in the UI.

### 2. What a person sets (Q101, Q122)

1. **Sandbox level**: **Full sandbox · Basic sandbox · No sandbox**. The config keys stay `on`,
   `floor` and `off`. Basic sandbox is the default: the scaffold no longer writes `mode = "on"`
   (Q114).
2. **Guards**, in Settings only, one switch per group, all on by default. The surface says a
   guard's effect, not its group's name (Q125).
   - **Secrets**: A2 (credentials). **A, the leak guard, is never switchable** (Q104).
   - **Project folder**: A3, A3b, A3c, A3d.
   - **Releases and merges**: A4.
   - **Issue and PR text**: A5, A6.
   - **Starting other chats**: A7.
   - **Always ask before**, each switched separately (Q98): merge · push to a protected branch or
     a bare force push · destructive MCP tools · held built-in tools.
3. **Who decides** (phase 3, Full sandbox only): **purlis decides**, or **{Harness} decides**.
   Each person confirms it per machine. A commit may forbid it, never switch it on (Q121).

**Every No sandbox chat** shows words on its mark in the attention colour, with "A chat runs as
you: it can change these settings and the local record of what it did". At No sandbox, guards
are described as catching accidents only (Q141).

**Never switchable**: a chat answering its own asks (V16a), the audit of changes, and the leak
guard.

### 3. Tiers and scopes (Q106, Q116, Q119, Q120)

| Tier | Where it's kept | What it can do |
|---|---|---|
| **Organisation policy** | `/etc/purlis/policy.json`, then an MDM profile | sets minimums per layer; enforced against everyone |
| **Project requires** | the committed `charter.toml` | sets minimums for chats (people can edit the file; review is the forge's) |
| **Project recommends** | the committed `charter.toml` | offers a level, guards and holds, which each person accepts or keeps their own |
| **Person** | this machine (ADR 0096) | sets their own, inside the bounds |
| **Chat** | app memory, for the chat's life (it survives the chat's restart) | "Change for this chat only", inside the bounds |

1. **Inside the bounds, the narrowest scope wins**: chat, then persona, then project.
2. **A timed exception** (Q119). At the project tier a person can declare one themselves: this
   chat only, with a reason, ending when the chat closes or at an hour they set. It is audited
   and shown on the mark. At the organisation tier it exists only if the policy allows exceptions
   and sets a maximum length. There is no approver yet, and the UI says so.
3. **The surveillance line** (Q120). At the project tier, nothing about a person leaves their
   machine. "Below your project's recommendation" shows only in the person's own Sandbox view. The
   organisation's remote sink (phase 4) sends effective protection and switch events, never
   content, and the Sandbox view lists what is sent and to whom.

### 4. Setup and change (Q115, Q117, Q118) — provisional UI

1. **At setup**, first run and New project get one row, with Basic sandbox preselected:
   "Protection: Basic sandbox (recommended) · Full sandbox · No sandbox", one line of help per
   option, and "Change any time in Settings". The main button still finishes in one press. "For a
   team" (folded) adds **Recommend to teammates** and **Require it**. Choosing No sandbox shows
   the vault-move and warning lines inline. **Joining** an existing project shows "This project
   recommends Full sandbox · Use it / Keep mine".
2. **Every change goes through one sheet, with one press.** It shows what changes; **For: me ·
   recommend to the team · require for the team**; the affected chats, with **Restart now**; the
   hosts to allow up front (Q140); and any vault move.
3. **Raising** acts at once. **Lowering** uses the danger style, never takes default focus, goes
   through the settle guard (ADR 0097), and applies only once its few-second **Undo** has passed.
   Settings always has a way back down (D-SE17g is retired). Level changes apply at each chat's
   next start.
4. **Committed protection settings have one change protocol** (Q117). It covers the level,
   guards, holds, hosts, `[risk]` protected branches, MCP entries and skills with hooks:
   1. A **tightening** applies on pull, with one Updates › Project line naming the commit and its
      author.
   2. A **loosening** counts only from a commit purlis itself fetched from upstream, read as
      [ADR 0067's amendment 2026-10-15-04](0067/2026-10-15-04-the-team-s-off-is-named-only-from-objects-checked-against-their-ids.md)
      reads it, and only after **each person's Accept** (or Keep mine). Each answer is bound to
      what was accepted. A local-only loosening changes nothing, and says "local".
   3. Every write through Settings records a **reason** in its commit message. A hand edit shows
      "no reason given".
   4. A loosening is never marked as already seen on a fresh clone (ADR 0119 decision 10).
   5. Review is the forge's. `purlis doctor` reports whether `charter.toml` has an owner rule.

### 5. Each harness's own check (Q96)

| Harness | Its own check | What purlis hears |
|---|---|---|
| Claude Code | a permission prompt plus the auto-mode classifier | prompts via the hook; blocks via `PermissionDenied`; the mode from the armed hook |
| opencode | permission asks, held in the window | the asks; no denial signal |
| Codex | approval stops, which purlis rejects | reports bypass for every chat; no denial signal yet |

On Codex and opencode, and on any chat at No sandbox, a guard's report comes from inside the
chat's reach and is tagged **unverified**. Unverified reports never reset or suppress a count.
Their row text is built by the app from fixed words, capped and collapsed ("12 unverified")
(Q133).

### 6. Attendance (Q142)

A chat is **unattended** if purlis started it unattended (a task, loop or runner), **or** if it
reports a mode purlis didn't set. A chat at Full sandbox with purlis deciding stays attended. A4,
dispatch, the briefing and the commit gate read this fact, never the harness's mode. At No
sandbox, an unattended task can't dispatch to *another* persona (as shipped).

### 7. Holds (Q73, Q135-Q137, Q74, Q98, Q58, Q76)

A **hold** is an action purlis blocks and turns into an ask.

1. **The app performs merges and held pushes itself.** There are no grants for them. A chat never
   merges or enqueues, and the rule that a chat may land after the person says "go" in it is
   retired (Q9, Q10, Q93).
2. **Merges** (Q136):
   - only repos in the project's inventory can be merged;
   - title, author, base and checks are read from the forge, never from the chat;
   - the row names the repo and the head SHA, and says "There is no undo";
   - **one press**, but only when the head is the SHA the person last viewed in purlis, or one
     with an approving review; otherwise the once-more press, with "2 commits since you looked";
   - a PR this chat didn't push is flagged;
   - a head that moves redraws the row;
   - a refusal by the ruleset or the queue shows on the row.
3. **Pushes**: the row shows the resolved URL, the target ref, and the old and new SHA.
   `--force-with-lease` to the chat's own branch isn't held.
4. **Declared merge commands** (Q137). A project lists its merge scripts (for example `node
   tools/land.mjs`) so the merge hold recognises them. That list, or a brokered forge token, is a
   precondition for offering "purlis decides".
5. **Grants exist only for MCP tools and held built-in tools.** A grant is one use, keyed on the
   hash of the raw argument bytes, with the full arguments shown. It expires at the end of the
   turn or after 10 minutes, whichever comes first. On Allow, purlis wakes the chat over the
   socket with the grant id and no prose. A grant shows in the Allowed list until it's used or
   expires.
6. **Risk classes.** Merge is a new class. A push maps to protected push, and a destructive MCP
   tool to Delete. Holds are never grouped, never a rule, and skipped by keyboard triage. The
   merge hold takes precedence over A4.
7. **Merge protection** (Q127). Whether a ruleset protects the branch is checked with the app's
   own forge login. It shows on the merge row and in the Sandbox view, and becomes a Notice only
   when a policy requires a ruleset. "No ruleset here, that's fine" is bound to what purlis saw at
   the time.
8. **A stated gap:** an undeclared script that merges with the chat's token gets past purlis.

### 8. MCP servers (Q128, phase 3)

1. **The launch is approved before the first start**: command, arguments, the names of its
   environment variables, and the binary's digest. Per project.
2. Its tools are listed **inside the sandbox**.
3. purlis **proposes** a sort into read · write · destructive · **asks every time**. The last is
   the default for unknown tools and tools with free-text arguments. Annotations are only hints.
4. One **Review** ask, with Accept. Sorting by hand happens in Settings.
5. **The tools list is pinned by its hash.** A change asks again, with a diff.
6. A launcher that can't be pinned (`npx …@latest`) reads **Can't pin**, and its writes always
   ask.
7. Before phase 3, the harness's own check judges MCP tools at every level.

**Built-in tools**: an explicit list, held by a pinned fixture per harness version plus a test.
NotebookEdit gets the edit hook.

### 9. Blocks (Q68, Q124-Q126, Q129-Q133, Q139)

A **block** is a refusal that doesn't stop the chat on its own. It comes from a guard, the
harness's own check, the sandbox, a policy or a project setting.

1. **Where blocks show:**
   - **Never in Asks**, which holds asks only.
   - A **marker on its chat**: "2 blocked", on the sidebar row, the pane band and the All chats
     row. It counts since the person last looked, and clears once the chat has been on screen or
     its Record has been opened. It's at the "Just so you know" level: no notification, no
     announcement, no effect on Quiet or snooze.
   - **The turn's finish row** in Updates › Mine gains "· 1 blocked: {source}", but only when the
     blocked action is still unresolved when the turn ends (Q131). It's one row per turn, never a
     failure notification, and identical rows are grouped across chats. Quiet's summary has a
     "stopped after a block" bucket.
   - **Rows say what happened**: "purlis blocked a force push". The group's name appears behind
     **Show this guard**.
2. **Tags** (Q124) appear when the source is the Sandbox, {Harness}'s own check, Organisation
   policy, Project requires or Project recommends, **unverified** or **Can't tell**. purlis's own
   consents carry none. Every row's detail always shows its source.
3. **Ways out:**
   - **A guard block:** Tell the chat… · Show this guard. Show this guard offers **Turn off for
     this chat**, through the change sheet. There's no Allow on the row.
   - **A block by the harness's own check:** Tell the chat… · Show this check, with "purlis can't
     allow this; only {Harness}'s own check decides". There's no Copy command.
   - **A sandbox block:** its Allow, as an ask while the chat waits · Show this rule.
   - **A policy or project block:** the tier, the file and the `owner` contact, and the timed
     exception where it exists.
   - **Can't tell:** Go to chat.
4. **Teaching** (Q126). After the same block repeats within a week, the Sandbox view shows
   "Blocked the same thing 12 times in 4 chats · **Tell chats here not to try**". That adds a
   standing instruction to the project's chat brief, which the person can undo. It never offers
   to switch a guard off.
5. **Other stops:**
   - A **hook give-up** (TooBig, NotItsHarness, TooLate, the 32-open cap) raises "Waiting in its
     terminal", with a test for each case (Q27).
   - The **sandbox consent** is only in Asks while it can be answered (Q107).

### 10. Loops (Q133, Q72)

| Rule | Threshold | What counts | Reset by |
|---|---|---|---|
| Block loop | 3 blocks on **the same target** (the tool plus its argument digest) | guard, sandbox, policy and project blocks; the harness's own check only where the harness has no fallback; never unverified reports | **any different successful call**, or an Allow |
| Your own refusals | 6 (as shipped) | your Don't allows | an Allow |
| Identical failure | 6 (as shipped) | the same command failing | another command, or success |
| Cool-off | 2 Don't allows on the same request | — | an Allow, or an hour |

1. A block that has an open ask never counts.
2. The pause is an ask: "steward 3 was blocked 3 times on the same thing. Let it keep trying?"
   Its answers: **Tell it to stop trying** (the lead) · **Let it keep trying** · **Stop chat**.
   Let it keep trying resets the count.
3. The count is of blocks as hook events and the target's digest. It never reads the text of a
   block's reason (ADR 0076).

### 11. Observability (Q129, Q130, Q134)

1. **One record.** Blocks, holds, grants, loop pauses, protection changes and answers to asks
   are **kinds of event in the existing event log**, which keeps argument digests and never
   commands. The Blocked log (kept 30 days), the fleet counts and each chat's timeline are views
   of it. No new store holds them.
2. **A Record tab per chat** (phase 1) is the one home for its story. It's pull-only: no count,
   no New marker, no notification. It opens from the mark's popover, the chat row, Blocked log
   rows, block results and the session record.
   - The start row records the effective protection per layer, which tier set it, the
     `charter.toml` commit, and the Accepts in force.
   - Its rows: protection changes and who made them · asks with their answers and who answered ·
     blocks with their source · holds and grants · loop pauses · the end and its result.
   - Allowed calls collapse into one line ("212 allowed · 9 by your rules").
   - The honesty lines (unverified; "ran as you") head the tab.
3. **Protection changes** are kept in an append-only local history beyond 90 days, since it
   holds no commands.
4. **All chats** shows the mark on every row, filters for No sandbox and unattended, and a
   weekly line per project with blocks per source, each linking to the Blocked log.

### 12. Surfaces — provisional UI

| Thing | Where |
|---|---|
| An ask: sandbox, harness prompt, consent, hold, loop pause, budget, hook give-up | **Asks (✋)** |
| A block | the chat's marker; the turn's finish row; the Blocked log; the Record |
| A finished turn or a failed chat | **Updates (🔔) › Mine** |
| A teammate's protection change, with its Accept | **Updates › Project** |
| A standing fault: mode differs, "purlis decides" forbidden, a broken policy, a ruleset a policy requires | **Notices** |
| What protects a chat, the locks, the Blocked log, the Allowed list with Remove, the teaching lines, "below your project's recommendation" | the right-bar **Sandbox** view (shield icon), read-mostly. Tooltip: "What protects my chats, and what did it block?" |
| Level, guards, Always ask before, hosts, folders & ports, MCP servers, built-in tools, project recommends and requires | **Settings › Project › Sandbox**, one page (Q102) |
| A chat's story | its **Record** tab |
| A chat's level | **the mark**: an icon only at the default; words when lower, pending, different from what purlis set, or at No sandbox. It sits in a fixed slot no pane drops, on the pane head, the sidebar row, the title bar and All chats. Its popover offers **Change for this chat only**, **Open the Sandbox view** and **Open the Record** |
| The pane's notes | **Pane notices**, with a new icon |

### 13. Allowing and remembering (Q83, Q84) — provisional UI

There is one **Allowed** list, with the verb **Remove**. The shipped Allow labels stay, in a
fixed order, and so do the shipped words for no: Keep blocked · Don't allow · Not now.

### 14. Hosts (Q103, Q140)

Hosts are recorded at every level; at No sandbox they're marked unverified. The Full sandbox
preview **pre-ticks** only preset hosts and hosts reached by declared tools. Every other host
starts unticked, with when it was first seen, how often and by which chat. Raw IPs, tunnels and
paste sites are never offered.

### 15. Policy (Q109-Q111, Q138, Q87)

1. Policy sets **minimums per layer**: level, required guard groups and holds, who decides,
   allowed rule scopes, dispatch locks, allowed harnesses and MCP servers (by identity), egress
   presets and hosts, retention, whether exceptions are allowed and their maximum length,
   `min-version`, and `owner` as the contact.
2. Between the file and MDM, the **strictest value per key** wins, and only forced managed
   values are read.
3. The **last-known-good** copy is kept in a keychain item bound to the app binary, with an age
   limit and an audit entry.
4. If the policy's `min-version` is above this install, or its schema is newer, **chats are
   refused**.
5. If no good copy ever existed, every lock is set (as shipped).
6. A required layer that a harness can't enforce refuses a chat on that harness at start, naming
   the policy. `purlis policy check` prints the harness matrix against the policy.
7. Every locked control, and every row blocked by a lock, names the tier, the file and the
   `owner`.
8. It is **called "Local policy locks"** until a remote audit sink exists (phase 4). That sink
   stores the policy text alongside its hash, which covers the organisation file, the project's
   sandbox section and the harness's managed settings, and is recorded at chat start and on every
   change.
9. **Stated limits:** a local admin can change root-owned files; a No sandbox chat runs as the
   person; a harness run outside purlis isn't governed, so admins should also deploy the
   harness's own managed settings.

### 16. Phases (Q112)

1. **Phase 1:** the fixes; the baseline fix (Q114) and the setup row (Q115); event-log kinds for
   blocks; guard denies reported, with the marker, the finish-row suffix and the Blocked log;
   `PermissionDenied` on Claude Code; "Blocked lately" folded in; the Record tab; the loop rule;
   the app-performed merge hold; attendance (Q142). The asks-scopes tickets 01, 02, 06 and 07
   land before phase 1's UI slices.
2. **Phase 2:** app-performed pushes; the Sandbox view and the Allowed list; guard switches, with
   `OVERRIDE_NOTE` rewritten; the one Settings page and the change sheet; the project tier, with
   the one change protocol; the timed exception; teaching; the mark and the renames; All chats.
3. **Phase 3:** MCP launch approval, review and pinning; the built-in tools table; declared merge
   commands; **purlis decides** at Full sandbox, confirmed per person per machine, with a preview
   of what stops being checked.
4. **Phase 4:** the policy keys; MDM, with the strictest value per key; the keychain-held
   last-known-good copy; the ruleset Notice; the remote audit sink; the harness matrix as a
   release gate.

The three fixes that come first (Q26) are each on their own tested branch: #6315 (`cafbeec92`),
the ✋ count of the sandbox consent (`f5bb1e413`), and #5242's secret-fill case (`f3dca8864`).

## Consequences

### The harness matrix

Each layer, by harness, is **enforced** (purlis holds it outside the chat's reach), **unverified**
(it is reported from inside the chat's reach, so it is shown and never trusted to reset or
suppress a count) or **Can't tell** (the harness reports nothing purlis could read). **Every cell
gets a test**, which fails when the harness stops behaving as its cell says, and a cell changes
only with its test. At No sandbox every report on every harness is unverified (decision 5), so
the table is for Basic and Full sandbox.

| Layer | Claude Code | Codex | opencode |
|---|---|---|---|
| Sandbox: files and egress | enforced (Bash in the sandbox; file tools by deny rules; web tools denied) | enforced (the whole process, its hooks and MCP servers, in purlis's wrap) | enforced (the whole process in purlis's wrap) |
| A guard's block reported | enforced (the hook runs outside the chat's reach) | unverified | unverified |
| The harness's own check's block reported | enforced (`PermissionDenied`) | Can't tell (no denial signal yet) | Can't tell (no denial signal) |
| The harness's prompts and asks reach Asks | enforced (the hook) | Can't tell (rejected, never asked) | enforced (held in the window) |
| The mode the chat runs in (attendance) | enforced (the armed hook) | Can't tell (bypass reported for every chat) | Can't tell |
| A hold the app performs (merge, push) | enforced | enforced | enforced |
| A non-purlis MCP tool's hold (phase 3) | enforced (the hook; the server runs outside the sandbox) | unverified (the hook runs in the wrap) | unverified (the hook runs in the wrap) |
| A held built-in tool (phase 3) | enforced (the pinned fixture) | Can't tell | Can't tell |
| Hosts recorded | enforced | enforced | enforced |
| The block loop's count | enforced | enforced for sandbox, policy and project blocks; unverified guard reports never count | enforced for sandbox, policy and project blocks; unverified guard reports never count |

`purlis policy check` prints this table against a policy (decision 15.6), and in phase 4 it is a
release gate.

### Otherwise

- Guard A4 reads attendance, not the harness's mode, so a Full sandbox chat run by purlis
  deciding keeps the release floor off while the person attends it, and an unattended task keeps
  it on.
- Until declared merge commands (phase 3), `node tools/land.mjs` and any script like it pass
  purlis, and the merge hold sees only what the guards already read. The ruleset check (decision
  7.7) is the real lock meanwhile, and the Sandbox view says so.
- The classifier's 59 false positives end only where the person chooses purlis decides at Full
  sandbox (phase 3). Before it, every Claude Code chat keeps the harness's own check, and its
  blocks are seen, tagged and counted instead of silent.
- The event log gains kinds; no store is added (ADR 0069). The chat tier's "Change for this chat
  only" lives in app memory; protection changes are an append-only local history; the
  last-known-good policy copy is a keychain item. Each names its tier in `docs/plane-format.md`
  before it ships, with the test that holds it.
- The scenario specs find every block from its chat's marker and every hold from the ✋, and each
  hook give-up has a test of its own.

## What it amends

- **[ADR 0067](0067-a-chat-runs-in-a-sandbox-charter-compiles-for-its-harness.md)** (the sandbox):
  the level's badge becomes the mark (decision 12); the floor, Basic sandbox, is the default and
  the scaffold stops writing `mode = "on"` (decision 2.1); team mode becomes the project tier,
  recommends and requires, under one change protocol (decisions 3 and 4.4); hosts are recorded at
  every level and allowed up front with care (decision 14).
- **[ADR 0090](0090-agents-work-together-as-chats-that-are-listed-observable-and-stoppable-and-no-agents-word-is-consent.md)**
  (tasks and dispatch): a task's mode follows its level, as a chat's does, in place of the cap at
  auto of [its amendment of 2026-10-29](0090/2026-10-29-a-task-starts-in-its-asking-chat-s-mode-capped-at-auto.md)
  (Q13); dispatch, the briefing and the commit gate read attendance, not
  `bypassPermissions` (decision 6); the mark reads the mode the armed hook reports
  ([amendment of 2026-10-30](0090/2026-10-30-a-reported-mode-is-believed-only-from-the-chat-s-armed-hook.md)).
- **[ADR 0096](0096-your-sandbox-level-is-yours-and-is-kept-on-this-machine.md)** (the level is
  yours): the three levels are renamed (decision 2.1); decision 7's Confirm of a lower level is
  merged into the change sheet, whose lowering waits out its Undo (decision 4.3); the record of
  what was confirmed, which no chat writes, now also holds guard switches and who decides.
- **[ADR 0119](0119-the-inbox-is-the-persons-own-in-three-views-on-the-right-bar.md)** (the right
  bar): the right bar gains the **Sandbox** view, read-mostly, with the shield icon
  (decision 12); its decision 10's first-open baseline never marks a loosening as seen
  (decision 4.4).
- **[ADR 0121](0121-attention-what-waits-on-you-reaches-you-and-nothing-else-interrupts.md)** and
  its amendments (attention):
  - blocks are not in Asks; they are a marker at the "Just so you know" level (decision 9);
  - [budgets and loops](0121/2026-11-10-budgets-and-loops-pause-into-asks.md): looping gains the
    block loop on the same target, and denials the sandbox or a guard makes are now counted
    (decision 10);
  - [risk as built](0121/2026-11-10-risk-is-shown-as-built.md): merge is a risk class, a push maps
    to protected push and a destructive MCP tool to Delete (decision 7.6);
  - [consents and confirmations](0121/2026-11-10-consents-and-confirmations-are-asks.md): the
    sandbox consent is in Asks only while it can be answered (decision 9.5);
  - [keyboard triage](0121/2026-11-10-keyboard-triage-and-the-asks-keyboard-ruling.md) skips
    holds (decision 7.6).
- **[ADR 0112](0112-one-notice-for-one-need-a-person-may-allow-an-undeclared-program.md)**
  (grants): a one-use grant exists only for MCP tools and held built-in tools; merges and held
  pushes are performed by the app and never granted (decision 7.5).
- **[ADR 0093](0093-a-blocked-local-socket-or-system-service-is-asked-about-like-a-host.md)** (the
  consent): what a project expects is answered in one place (decision 9.5).
- **[ADR 0069](0069-every-store-charter-writes-is-in-one-of-four-tiers.md)** (tiers of stores):
  the new state is placed as [Consequences](#otherwise) says, and each piece names its tier
  before it ships.
- **[ADR 0076](0076-a-run-moves-only-by-a-named-cause-and-a-chats-state-is-read-from-its-runs.md)**
  (no parsing): the loop counts hook events and argument digests, and a block's `reason` is shown
  as written and never read (decision 10.3). A loop pause is a named cause.
- **Toolgate's "no switch" ruling** (`toolgate::OVERRIDE_NOTE`: "there is deliberately no config
  key, environment variable or switch that lifts a purlis denial: one purlis could read is one a
  committed file could flip") is reversed. Its reason no longer holds: a guard switch counts only
  once confirmed in the window, through a record no chat writes (ADR 0096 decision 7), a commit
  can only tighten it, and a policy can lock it (decisions 2.2 and 4.4). `OVERRIDE_NOTE` is
  rewritten in phase 2.

## Not chosen

Each was agreed in an earlier round and later revised; decisions.md keeps the history.

- **Presets** (Relaxed, Balanced, Strict, Standard, Custom; Q31, Q59, Q77). Dropped by Q101: a
  person sets the level, the guards and who decides.
- **Copy command** on a block by the harness's own check (Q61). Dropped by Q139: the row says
  "purlis can't allow this; only {Harness}'s own check decides".
- **Push grants** (Q73, Q99). Replaced by Q135: the app performs the push it shows, and grants
  exist only for MCP tools and held built-in tools.
- **"MDM wins"** over the policy file (Q110). Replaced by Q138: the strictest value per key wins.
- **A first-run picker** (Q32, Q64, Q78). Replaced by Q115: one Protection row at setup, with
  Basic sandbox preselected, which finishes in one press.
- **Blocks in Asks**, as a Refused line (Q3). Revised by Q68: Asks holds asks only, and a block is
  a marker on its chat.
- **The one-press merge without a viewed SHA** (Q97). Refined by Q136: one press only when the
  head is the SHA the person last viewed, or one with an approving review.

## Amendments

An amendment to this decision is a file of its own in `0124/`, named `<yyyy-mm-dd>-<slug>.md`,
never a section added to this record (#1982). The first expected is the one after the person's
review of the prototype, on the UI sections.
