# 0124. Who judges a chat, and how much

Status: proposed (2026-11-12). The person's design, agreed over decisions Q1-Q184 in four review
rounds and a final UI pass against the prototype on 2026-11-12, with Q185-Q215 decided by the
orchestrator for the person's review. Amends ADRs 0067, 0069, 0076, 0090, 0093, 0096, 0112, 0119
and 0121 and its amendments, and reverses toolgate's "no switch" ruling (see
[What it amends](#what-it-amends)).

**The UI is settled.** The final pass against prototype v3 (branch
`prototype-v2-who-judges-a-chat-from-desi-wc7ypwec`, commit 5c5b8df, 20 stories) found 27
friction points, and Q157-Q180 resolve them. Where those answers change an earlier one, this
record states the later.

**Decided by the orchestrator, for the person's review.** The person handed the open choices that
the ledger's coverage check, the dedup pass and the slicing of phase 1 found to the orchestrator
("All you should do yourself"). Each was decided by its recommended answer, and each is marked
here as *decided by the orchestrator, for the person's review*. Any of them can be overturned by
an amendment.

**The ledger.** Each decision section names the requirements it states, by their R-ID in the
requirements ledger v2 (352 rows, R-001 to R-352), which is kept with the design notes outside the
repo. A requirement is built and tested from the ledger; this record says why.

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
  A `terminal` ask is answerable only in its pane. A refusal whose only way out is a press in the
  window says so in words and gives no control: a chat's GitHub token could not be minted
  because its clone was not in purlis's clone record, and the only way to trust that clone was a
  Notice that can't be searched for and stays hidden once dismissed (found while this design was
  being built).
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
sandbox, forge controls that only read words, and built-in tools no hook sees. A final pass
settled the UI against a prototype. This record carries the last state only.

## The decision

### 1. Principles

Ledger: R-001 to R-010, R-315.

1. **Protection when you need it** (Q113). purlis *can* make an agentic coding environment
   secure. Protection is chosen when a project is set up and is easy to change later in
   Settings; in phase 1, Settings is the shipped sandbox page. The goals, all at once: user
   friendly, secure, governed, controllable, observable, not noisy, and basic on the surface with
   depth underneath.
2. **The one-sentence model.** *purlis keeps each chat inside limits that you, your project or
   your organisation choose. When a chat reaches one, purlis asks you, or blocks it and tells
   you, and it always says what stopped it and what you can do.*
3. **The invariant** (Q1). Every stop of a chat shows up in exactly one place, says who stopped
   it, and offers a way out. No black holes, no dead loops. A block's one place is its row in the
   chat's **Journal** (decision 11); the marker, the finish row and the Blocked log point to it
   or are views of it (Q166, Q176).
4. **Harness-agnostic** (Q67, Q88). Every concept is purlis's, defined in core, and each harness
   maps onto it only through its adapter. Harness-specific names (`bypassPermissions`, `auto`,
   `PermissionDenied`) appear only in adapters and ADR notes, never in the UI; `{Harness}` is
   filled from the adapter's display name. Where a harness can't report something, the UI says
   **Can't tell**. This record carries a matrix of layers by harness, with one test per cell
   ([Consequences](#the-harness-matrix)).
5. **Honesty** (Q59, Q76, Q109, Q139, Q141). Nothing promises more than it enforces, and each
   known gap is stated in the UI, shipping with the feature it belongs to.
6. **Words follow the copy guard** (Q180). View names (the Sandbox view, the Journal, the Blocked
   log, Allowed) are reworded to `docs/ui-copy.md`'s rules as each is built.

### 2. What a person sets (Q101, Q122)

Ledger: R-011 to R-037, R-320, R-324, R-326, R-345.

1. **Sandbox level**: **Full sandbox · Basic sandbox · No sandbox**. Only the names change; the
   files keep their values. `charter.toml`'s `mode` has no `floor` value: with no `mode`, a chat
   starts at the person's own level, which defaults to Basic sandbox (the floor), and that is
   pinned by a test. The scaffold no longer writes `mode = "on"` (Q114; Q201 and Q215, decided by the
   orchestrator, for the person's review).
2. **Guards**, one switch per group, all on by default. A guard switch is the person's own, kept
   on this machine (ADR 0096) inside the bounds, and counts only once confirmed in the window,
   through a record no chat writes (Q28). The person's and the project's switches are edited in
   Settings only; a change for one chat comes from the mark's popover or Show this guard,
   through the same change sheet. The surface says a guard's effect, not its group's name
   (Q125).
   - **Secrets**: A2 (credentials). **A, the leak guard, is never switchable** (Q104).
   - **Project folder**: A3, A3b, A3c, A3d.
   - **Releases and merges**: A4.
   - **Issue and PR text**: A5, A6.
   - **Starting other chats**: A7.
   - **Always ask before**, each hold switched separately (Q98): merge · push to a protected
     branch or a bare force push (phase 2) · destructive MCP tools · held built-in tools (phase
     3).
3. **Who decides** (phase 3, Full sandbox only): **purlis decides**, or **{Harness} decides**.
   - It is confirmed per person, per machine and per project, since its preconditions and its
     preview are per project, and it is set on Settings › Project › Sandbox (Q121; Q192, decided
     by the orchestrator, for the person's review).
   - Confirming it shows a preview of what stops being checked.
   - A commit may forbid it, never switch it on.
   - When the harness's managed settings forbid it, the chat falls back to {Harness} decides
     (Q65).
   - A task's "who decides" follows the same rule as a chat's (Q13).
   - Until it is confirmed, a chat at every level keeps the harness's own check (Q55, Q64).

**Every No sandbox chat** shows words on its mark in the attention colour on the pane head, the
title bar, All chats and the popover, and on the sidebar the icon in the attention colour (Q190,
decided by the orchestrator, for the person's review, reconciling Q141 with Q177). Its popover
says "A chat runs as you: it can change these settings and the local record of what it did". At
No sandbox, guards are described as catching accidents only (Q141).

**Never switchable**: a chat answering its own asks (V16a), the audit of changes, and the leak
guard.

### 3. Tiers and scopes (Q106, Q116, Q119, Q120)

Ledger: R-038 to R-058, R-305, R-321.

| Tier | Where it's kept | What it can do |
|---|---|---|
| **Organisation policy** | `/etc/purlis/policy.json`, then an MDM profile | sets minimums per layer; enforced against everyone |
| **Project requires** | the committed `charter.toml` | sets minimums for chats (people can edit the file; review is the forge's) |
| **Project recommends** | the committed `charter.toml` | offers a level, guards and holds, which each person takes (**Use it**) or declines (**Keep mine**) |
| **Person** | this machine (ADR 0096) | sets their own, inside the bounds |
| **Chat** | app memory, for the chat's life (it survives the chat's restart) | "Change for this chat only", inside the bounds |

1. **Inside the bounds, the narrowest scope wins**: chat, then persona, then the person's project
   value.
2. **A chat-only change** reads "Changed for this chat only, by you" (Q171). It is audited, and
   never inherited by the chats that chat starts (Q37-Q41).
3. **A timed exception** (Q119, Q167). A person can declare one themselves, for this chat only,
   with a reason. It ends when the chat closes or at an hour they set, and survives the chat's
   restart (a restart is not a close). It is audited and shown on the mark.
   - Against **Project recommends** it is always possible.
   - Against **Project requires** it is possible only if `charter.toml` says
     `exceptions = "self-declared"`, which is off by default.
   - At the organisation tier it exists only if the policy allows exceptions and sets a maximum
     length.
   - There is no approver yet, and the UI says so.
4. **The surveillance line** (Q120). At the project tier, nothing about a person leaves their
   machine. "Below your project's recommendation" shows only in the person's own Sandbox view. The
   organisation's remote sink (phase 4) sends effective protection and switch events, never
   content, and the Sandbox view lists what is sent and to whom.

### 4. Setup and change (Q115, Q117, Q118)

Ledger: R-059 to R-092, R-293 to R-298, R-306, R-307, R-317.

1. **At setup**, First run and New project get one row, with Basic sandbox preselected:
   "Protection: Basic sandbox (recommended) · Full sandbox · No sandbox", one line of help per
   option, and "Change any time in Settings".
   - Full sandbox's line says "New hosts are asked about the first time a chat reaches them"
     (Q159).
   - The main button finishes setup in one press at every choice, No sandbox included (Q156).
     Choosing No sandbox shows the vault-move and warning lines inline, with no extra step, and
     the button says what it does ("Create, and move 2 vaults to the keychain").
   - No sandbox is refused when the project has a team-registered vault: the refusal names the
     vault's owner and offers Basic sandbox (Q163). The change sheet refuses it the same way.
   - On New project, a folded "For a team" section adds **Recommend to teammates** and
     **Require it** (phase 2). It isn't offered at First run (Q158).
   - **Joining** an existing project shows "This project recommends Full sandbox · Use it / Keep
     mine" (phase 2). Wherever a person answers a setting the project offers (joining, a
     recommendation, a teammate's loosening), the answers are **Use it / Keep mine** (Q157).
2. **Every change goes through one sheet, confirmed with one press.** It shows:
   - what changes;
   - who it is for. Raising offers **me · recommend to the team · require for the team**. A team
     lowering offers **me · recommend to the team · stop requiring**, never "Require" (Q160);
   - a one-line reason, required when the change is for the team (Q161);
   - the affected chats, with **Restart now**;
   - the hosts to allow up front (Q140);
   - any vault move.
3. **Raising** acts at once: it is written on confirm, with no Undo wait. **Lowering**:
   - uses the danger style;
   - never takes default focus;
   - goes through the settle guard (ADR 0097);
   - applies only once its **Undo** toast, shown for 5 seconds, has passed. After that, the
     change's row in the chat's Journal offers **Revert** (Q162).

   Settings always has a way back down.
4. **Lowering is never a switch** (Q172). A control that is on reads "On · Turn off…" and opens
   the sheet. Turning a guard or a hold on is a switch, and it acts at once.
5. **When a change applies.** Guard and hold changes apply to running chats at once; the level
   applies at each chat's next start (Q37-Q41).
6. **Committed protection settings have one change protocol** (Q117). It covers the level,
   guards, holds, hosts, `[risk]` protected branches, MCP entries, skills with hooks, and
   `exceptions = "self-declared"`. Anything that is not provably a tightening is a loosening.
   1. A **tightening** applies on pull, with one Updates › Project line naming the commit and its
      author.
   2. A **loosening** counts only from a commit purlis itself fetched from upstream, read as
      [ADR 0067's amendment 2026-10-15-04](0067/2026-10-15-04-the-team-s-off-is-named-only-from-objects-checked-against-their-ids.md)
      reads it, and only after **each person's Use it** (or Keep mine). Each answer is bound to
      what was accepted. A local-only loosening changes nothing, and says "local".
   3. While it waits for the person's answer, a teammate's loosening is an amber **Notice** with
      Use it / Keep mine. Once answered, it is history in Updates › Project (Q179).
   4. Every write through Settings records a **reason** in its commit message. A hand edit shows
      "no reason given".
   5. A loosening is never marked as already seen on a fresh clone (ADR 0119 decision 10).
   6. Review is the forge's. `purlis doctor` reports whether `charter.toml` has an owner rule.

### 5. Each harness's own check (Q96)

Ledger: R-093 to R-105, R-325.

| Harness | Its own check | What purlis hears |
|---|---|---|
| Claude Code | a permission prompt plus the auto-mode classifier | prompts via the hook; blocks via `PermissionDenied`; the mode from the armed hook |
| opencode | permission asks, held in the window | the asks; no denial signal, so its blocks read Can't tell |
| Codex | approval stops, which purlis rejects | reports bypass for every chat, so its mode reads Can't tell (Q152); no denial signal yet. Where a Codex hook reports a rejection, it becomes a block; otherwise the cell reads Can't tell (Q25) |

On Codex and opencode, and on any chat at No sandbox, a guard's report comes from inside the
chat's reach and is tagged **unverified**. Unverified reports never reset or suppress a count.
Their row text is built by the app from fixed words, capped and collapsed ("12 unverified") in
the chat's Journal and the detail view (Q133, Q165).

### 6. Attendance (Q142)

Ledger: R-106 to R-113, R-289, R-318.

A chat is **unattended** if purlis started it unattended (a task, loop or runner), **or** if it
reports a mode purlis didn't set.

- Codex's constant bypass report isn't a mode purlis didn't set: a Codex chat's attendance comes
  only from how purlis started it (Q152).
- Attendance moves one way: an unattended chat never becomes attended (Q105).
- A chat at Full sandbox with purlis deciding stays attended (phase 3).
- A4, dispatch, the briefing and the commit gate read this fact, never the harness's mode.
- At No sandbox, an unattended task can't dispatch to *another* persona (as shipped).
- Measuring Claude Code's live `permission_mode` strings (#3825) is the first step of the
  attendance work, and blocks it (Q198, decided by the orchestrator, for the person's review).

### 7. Holds (Q73, Q135-Q137, Q74, Q98, Q58, Q76)

Ledger: R-114 to R-151, R-287, R-288, R-302, R-303, R-348.

A **hold** is an action purlis blocks and turns into an ask.

1. **The app performs merges and held pushes itself.** There are no grants for them. A chat never
   merges or enqueues, and the rule that a chat may land after the person says "go" in it is
   retired (Q9, Q10, Q93).
2. **Merges** (Q136):
   - only repos in the project's inventory can be merged;
   - title, author, base and checks are read from the forge, never from the chat;
   - the row names the repo and the head SHA, and says "There is no undo";
   - **one press**, but only when the head is the SHA the person last viewed in purlis, or the
     current head SHA has an approving review, read from the forge, by an account that is
     neither the PR's author nor an app or bot (Q186, decided by the orchestrator, for the
     person's review). Otherwise it takes the once-more press, with "2 commits since you looked";
   - **View changes** on the row opens the PR's diff in purlis and marks its head SHA as viewed
     (Q168). The viewed SHA is kept in the app's memory, per PR, for the window's life, and
     never on disk: after a restart the person views again (Q210, decided by the orchestrator,
     for the person's review);
   - a PR this chat didn't push is flagged, by the pushes purlis recorded for the chat;
   - a head that moves redraws the row;
   - a refusal by the ruleset or the queue shows on the row;
   - the word for no is **Don't merge** (Q169).
3. **Pushes** (phase 2): the row shows the resolved URL, the target ref, and the old and new SHA,
   and its word for no is **Don't push** (Q169). A push to a protected branch, or a bare force
   push, is held; at No sandbox the hold still applies and is described as catching accidents
   only. `--force-with-lease` to the chat's own branch isn't held.
4. **Switching the merge hold off** (Q151) means purlis stops intercepting merges: the forge and
   the chat's own token decide. A4 still applies when the chat is unattended.
5. **Declared merge commands** (Q137). A project lists its merge scripts (for example `node
   tools/land.mjs`) so the merge hold recognises them. That list, or a brokered forge token (a
   later design, outside these phases), is a precondition for offering "purlis decides".
6. **Grants exist only for MCP tools and held built-in tools** (phase 3). A grant is one use,
   keyed on the hash of the raw argument bytes, with the full arguments shown. It expires at the
   end of the turn or after 10 minutes, whichever comes first. On Allow, purlis wakes the chat
   over the socket with the grant id and no prose. A grant shows in the Allowed list until it's
   used or expires.
7. **Risk classes.** Merge is a new class. A held push maps to protected push, and a destructive
   MCP tool to Delete. Holds are never grouped, never become a rule, and are skipped by keyboard
   triage. The merge hold takes precedence over A4; since that depends on attendance, it is
   built and tested with attendance (Q202, decided by the orchestrator, for the person's review).
8. **Merge protection** (Q127). Whether a ruleset protects the branch is checked with the app's
   own forge login. It shows on the merge row and, from phase 2, in the Sandbox view, and becomes
   a Notice only when a policy requires a ruleset (phase 4). "No ruleset here, that's fine" is
   bound to what purlis saw at the time.
9. **A stated gap**, shown beside merge protection: a script that merges with the chat's token
   gets past purlis. From phase 3 it reads "an undeclared script".

### 8. MCP servers and built-in tools (Q128, phase 3)

Ledger: R-152 to R-168, R-309, R-343, R-344.

1. **The launch is approved before the first start**, per project: command, arguments, the names
   of its environment variables, and the binary's digest. These four fields are the server's
   identity.
2. Launch approval and tool sorting apply at Full and Basic sandbox. At No sandbox the server
   runs as the person, and the hold is described as catching accidents only (Q191, decided by the
   orchestrator, for the person's review).
3. Its tools are listed **inside the sandbox**.
4. purlis **proposes** a sort of each tool into **Read · Write · Destructive**, with an **Always
   ask** checkbox per tool (Q174, revising Q128's "asks every time" bin). An unknown tool starts
   in Write with Always ask ticked, and Always ask is ticked by default for a tool with free-text
   arguments; annotations may suggest another bin, and are only hints (Q194, decided by the
   orchestrator, for the person's review).
5. A call to a tool sorted Destructive, or with Always ask ticked, is held as an ask and answered
   with a one-use grant (decision 7.6).
6. One **Review** ask, with Accept. Sorting by hand (each tool's bin and its Always ask) happens
   in Settings.
7. **The tools list is pinned by its hash.** A change asks again, with a diff.
8. A launcher that can't be pinned (`npx …@latest`) reads **Can't pin**, and its writes always
   ask: Always ask is ticked and locked on its Write and Destructive tools.
9. Before phase 3, the harness's own check judges MCP tools at every level.

**Built-in tools** are an explicit list, held by a pinned fixture per harness version plus a test.
Artifact publish, the schedule tools and remote agents are held as asks, answered with a one-use
grant. NotebookEdit gets the edit hook. In Settings the list is read-only; its group switch is the
only control (Q175).

### 9. Blocks (Q68, Q124-Q126, Q129-Q133, Q139)

Ledger: R-169 to R-205, R-290 to R-292, R-299 to R-301, R-316, R-340 to R-342, R-351.

A **block** is a refusal that doesn't stop the chat on its own. It comes from one of five
sources: a guard, the harness's own check, the sandbox, a policy or a project setting. Every
guard deny is reported to the app and recorded as a block. The word is "Blocked", never "Refused"
(Q70).

1. **Where blocks show:**
   - **Never in Asks**, which holds asks only.
   - A **marker on its chat**: "2 new blocked" (Q164), on the sidebar row and the pane band, and
     from phase 2 on the All chats row. It counts since the person last looked, and clears once
     the chat has been on screen or its Journal has been opened. A chat has been on screen when
     its pane is visible in the focused window, by the rule the shipped unread markers use (Q187,
     decided by the orchestrator, for the person's review). It's at the "Just so you know" level:
     no notification, no announcement, no effect on Quiet or snooze. It opens the chat's
     Journal, filtered to blocks (Q166).
   - Unverified reports never count on the sidebar: it shows a muted dot, and the count is in the
     Journal and the detail view (Q165).
   - **The turn's finish row** in Updates › Mine gains "· 1 blocked this turn" (Q164), but only
     when the blocked action is still unresolved when the turn ends: no later success on the same
     target and no Allow (Q131). It's one row per turn, never a failure notification, and
     identical rows are grouped across chats. Quiet's summary has a "stopped after a block"
     bucket.
   - **Rows say what happened**: "purlis blocked a force push". The group's name appears only
     behind **Show this guard**.
2. **Tags** (Q124) appear when the source is the Sandbox, {Harness}'s own check, Organisation
   policy, Project requires or Project recommends, **unverified** or **Can't tell**. purlis's own
   consents carry none. Tags are words with neutral icons, never emoji (Q71). Every row's detail
   always shows its source, and the harness's `reason` text as written, never parsed, while the
   app runs (Q185, decided by the orchestrator, for the person's review; it is not stored,
   decision 11.1).
3. **Ways out.** They are on the turn's finish row and on the block's row in the chat's Journal
   (Q166). In phase 1, Show this guard, Show this check and Show this rule open a read-only
   detail.
   - **A guard block:** Tell the chat… · Show this guard. From phase 2, Show this guard offers
     **Turn off for this chat**, through the change sheet. There's no Allow on the row.
   - **A block by the harness's own check:** Tell the chat… · Show this check, with "purlis can't
     allow this; only {Harness}'s own check decides". There's no Copy command.
   - **A sandbox refusal** (Q154) is an ask in Asks while the chat waits on the held connection,
     with its Allow. Once the chat has stopped waiting (a timeout, or it moved on), it is a
     block, whose ways out are **Allow next time** · Show this rule. Allow next time makes a grant
     without an open ask. It offers the shipped Allow labels, where "once" means the chat's next
     attempt, and the entry stays in the Allowed list until it is used or removed (Q188, decided
     by the orchestrator, for the person's review). In phase 1 it is listed where shipped grants
     are listed.
   - **A policy or project block:** the tier, the file and the `owner` contact, and the timed
     exception where it exists (project in phase 2, policy in phase 4).
   - **Can't tell:** Go to chat.
4. **Teaching** (Q126, phase 2). When the same source blocks the same target (the tool plus its
   argument digest) 3 times, across at least 2 chats, within 7 days (Q189, decided by the
   orchestrator, for the person's review), the Sandbox view shows "Blocked the same thing 12
   times in 4 chats · **Tell chats here not to try**". That adds a standing instruction to the
   project's chat brief, which the person can undo. It never offers to switch a guard off.
5. **Other stops:**
   - A **hook give-up** (TooBig, NotItsHarness, TooLate, the 32-open cap) is listed in Asks as
     "Waiting in its terminal", with a test for each case (Q27). It offers **Go to chat** only,
     and is answered in the pane (Q155).
   - A chat stopped at the harness's own **folder-trust prompt**, which can come before any hook
     loads, shows as "Waiting in its terminal" where purlis can tell, and otherwise as Can't
     tell on the chat's sidebar row, and from phase 2 in All chats. It never stands only in a
     dialog or a Notice (Q200; Q205 and Q214, decided by the orchestrator, for the person's
     review).
   - The **sandbox consent** is only in Asks while it can be answered (Q107).
6. **Existing issues re-scoped.** #1903, #1904 and #1906 asked for sandbox blocks to raise
   Notices. Their detection work stands; only the surface changes, to the ask or the block above
   (Q196, decided by the orchestrator, for the person's review).

### 10. Loops (Q133, Q72)

Ledger: R-206 to R-218, R-350.

| Rule | Threshold | What counts | Reset by |
|---|---|---|---|
| Block loop | 3 blocks on **the same target** (the tool plus its argument digest) | guard, sandbox, policy and project blocks; **never** the harness's own check (Q153), never unverified reports | **any different successful call**, or an Allow |
| Your own refusals | 6 (as shipped) | your Don't allows | an Allow |
| Identical failure | 6 (as shipped) | the same command failing | another command, or success |
| Cool-off | 2 Don't allows on the same request (ADR 0121's, built by the asks-scopes work) | the person's Don't allows | an Allow, or an hour |

1. A block that has an open ask never counts. The block loop and the cool-off are counted
   separately.
2. The pause is an ask: "steward 3 was blocked 3 times on the same thing. Let it keep trying?"
   Its answers: **Tell it to stop trying** (the lead) · **Let it keep trying** · **Stop chat**.
   Let it keep trying resets the count.
3. The count is of blocks as hook events and the target's digest. It never reads the text of a
   block's reason (ADR 0076).
4. **A known limit:** a reworded retry is a different target, so it starts a new count (Q133
   replaced Q8's "however it is worded").

### 11. Observability (Q129, Q130, Q134)

Ledger: R-219 to R-239, R-310, R-349.

1. **One record.** Blocks, holds, grants, loop pauses, protection changes and answers to asks
   are **kinds of event in the existing event log**, which keeps argument digests and never
   commands. A block event keeps the tool, the source, a **short target summary** (a host,
   folder, service or guard arm; never a command, its arguments or its output) and the argument
   digest, which the loop matches on (Q206, decided by the orchestrator, for the person's
   review). The harness's `reason` text is not stored, since it can quote a command (Q185). The
   Blocked log (kept 30 days), the fleet counts and each chat's timeline are views of the log.
   No new store holds them.
2. **The Blocked log** replaces the shipped "Blocked lately". In phase 1 it lives where Blocked
   lately lives today, and in phase 2 it moves into the Sandbox view. Folding Blocked lately in is
   a migration, built as expand (the new view reads both records), migrate, then contract (the
   old record retired) (Q207, decided by the orchestrator, for the person's review).
3. **A Journal tab per chat** (phase 1) is the one home for its story. It is named **Journal**
   ("Open the journal"): "Record" clashes with the shipped session record (Q211), and "Activity"
   with the shipped Activity view, the timeline of what a chat and its tasks said to each other
   through purlis (Q213, decided by the orchestrator, for the person's review: a UI name the
   person may want to choose).
   - It's pull-only: no count, no New marker, no notification.
   - It has its own simple timeline (Q176).
   - It opens from the chat row, Blocked log rows, block results, the session record and the
     marker, and from phase 2 from the mark's popover.
   - The chat's start event records the effective protection per layer, which tier set it, the
     `charter.toml` commit, and the answers in force. Its start row is a one-line summary, linking
     to the Sandbox view from phase 2 (Q176).
   - Its rows: protection changes and who made them (in phase 1 those made through the shipped
     paths) · asks with their answers and who answered · blocks with their source · holds and
     grants (grants from phase 3) · loop pauses · the end and its result.
   - Allowed calls collapse into one line ("212 allowed · 9 by your rules"). "By your rules"
     counts calls allowed by an entry in the Allowed list.
   - The honesty lines (unverified; "ran as you", on a No sandbox chat) head the tab.
4. **Protection changes** are kept in an append-only local history beyond 90 days, since it
   holds no commands (phase 2).
5. **All chats** (phase 2) shows the mark on every row, filters for No sandbox and unattended, and
   a weekly line per project with blocks per source, each linking to the Blocked log.

### 12. Surfaces

Ledger: R-240 to R-252, R-304, R-308, R-311 to R-314, R-319, R-346.

| Thing | Where |
|---|---|
| An ask: sandbox, harness prompt, consent, hold, loop pause, budget, hook give-up, a window-only refusal | **Asks (✋)** |
| A block | the chat's marker; the turn's finish row; the Blocked log; its Journal |
| A finished turn or a failed chat | **Updates (🔔) › Mine** |
| A teammate's tightening, and a loosening once answered | **Updates › Project** |
| A teammate's loosening waiting for your answer | a **Notice** (amber), with Use it / Keep mine |
| A standing fault: mode differs (phase 1), "purlis decides" forbidden (phase 3), a broken policy, a ruleset a policy requires (phase 4) | **Notices** |
| What protects a chat, the locks, the Blocked log, the Allowed list with Remove, the teaching lines, "below your project's recommendation", and the harness's managed settings purlis read | the right-bar **Sandbox** view (shield icon), read-mostly, in two sections: **This chat**, which follows the focused chat, then **This project** (Q170). Tooltip: "What protects my chats, and what did it block?" purlis never writes the harness's managed settings (Q48, Q49) |
| Level, guards, Always ask before, hosts, folders & ports, MCP servers, built-in tools, project recommends and requires | **Settings › Project › Sandbox**, one page (Q102). Hosts, folders and ports are folded below Full sandbox, with "Used at Full sandbox" (Q173). MCP servers and built-in tools arrive in phase 3 |
| A chat's story | its **Journal** tab |
| A chat's level | **the mark** (below) |
| The pane's notes | **Pane notices**, with a new icon |

**The mark** sits in a fixed slot no pane drops, on the pane head, the sidebar row, the title bar
and All chats.

- It is an icon only at the default and at Full sandbox, which is above the default (Q177).
- It shows words, on the pane head and in the popover, when the level is lower, pending,
  different from what purlis set, or at No sandbox. On the sidebar row it is the icon plus a
  pending dot (Q177); No sandbox is drawn there as decision 2 says.
- When "purlis decides" is on, it shows who decides: in words on the pane head and in the
  popover, and as the icon on the sidebar (Q193, decided by the orchestrator, for the person's
  review).
- A theme token, `attention.*`, colours No sandbox and pending changes (Q178).
- Its popover offers **Change for this chat only**, **Open the Sandbox view** and **Open the
  journal**.

### 13. Allowing and remembering (Q83, Q84)

Ledger: R-253 to R-256.

There is one **Allowed** list, with the verb **Remove**. The shipped Allow labels stay, in this
order: once · this chat · this project (me) · this project (everyone, committed) · this machine
(delivered by the asks-scopes work). The shipped words for no stay too (Keep blocked · Don't
allow · Not now), except on hold rows, which say Don't merge and Don't push (decision 7).

A wish for a new label, "Always allow for this persona" (#1407, and the same item in #1411), is
declined for now: the labels stay fixed. The issue stays open as a later amendment to this
record, if it is wanted (Q197, decided by the orchestrator, for the person's review).

### 14. Hosts (Q103, Q140)

Ledger: R-257 to R-261, R-352.

Hosts are recorded at every level; at No sandbox they're marked unverified. The Full sandbox
preview **pre-ticks** only preset hosts and hosts reached by declared tools. Every other host
starts unticked, with when it was first seen, how often and by which chat. Raw IPs, tunnels and
paste sites are never offered. One host record serves both that preview and Settings › Your hosts
(#6172; Q200, decided by the orchestrator, for the person's review).

### 15. Policy (Q109-Q111, Q138, Q87)

Ledger: R-262 to R-280, R-322, R-323, R-347.

1. Policy sets **minimums per layer**: level, required guard groups and holds, who decides,
   allowed rule scopes, dispatch locks, allowed harnesses and MCP servers (by identity), egress
   presets and hosts, retention, whether exceptions are allowed and their maximum length,
   `min-version`, and `owner` as the contact. An unknown key fails closed (Q45).
2. Between the file and MDM, the **strictest value per key** wins, and only forced managed
   values are read.
3. The **last-known-good** copy is kept in a keychain item bound to the app binary. Using it
   writes an audit entry. It expires after 30 days, which a policy key can override (Q195,
   decided by the orchestrator, for the person's review).
4. If the policy's `min-version` is above this install, or its schema is newer than this install
   knows, **chats are refused**.
5. If no good copy ever existed, every lock is set (as shipped).
6. A required layer that a harness can't enforce refuses a chat on that harness at start, naming
   the policy. `purlis policy check` prints the harness matrix against the policy.
7. Every locked control, and every row blocked by a lock, names the tier, the file and the
   `owner`. The project tier's locks do so from phase 2.
8. It is **called "Local policy locks"** from phase 2 until a remote audit sink exists (phase 4).
   That sink stores the policy text alongside its hash, which covers the organisation file, the
   project's sandbox section and the harness's managed settings, and is recorded at chat start
   and on every change.
9. **Stated limits**, each in the UI: a local admin can change root-owned files; a No sandbox chat
   runs as the person (from phase 2); a harness run outside purlis isn't governed, so admins
   should also deploy the harness's own managed settings.
10. Admins get a policy schema docs page and two examples, Enterprise strict and Regulated (Q51).

### 16. No window-only dead ends (Q181-Q184, phase 1)

Ledger: R-328 to R-339.

1. **A refusal whose only way out is a press in the window raises an ask**, with that control on
   its row (Q181). It is Blocking if a chat waits on it; otherwise it sits on the chat's finish
   row. No message says "from the window" without the control attached. For example, a GitHub
   token that can't be minted for an unrecorded clone raises "Trust this clone so chats can use
   GitHub", with **Trust this clone**.
2. **Notices are findable** (Q182): a search box, filters by kind, and groups with counts
   ("Clones made outside purlis · 3"). Every message that names a window control links to that
   control, which opens focused.
3. **Dismissed doesn't mean gone** (Q183): a "Dismissed (N)" filter lists dismissed Notices, and
   any of them can be brought back. A dismissed Notice that later stops a chat is raised again, as
   an ask.
4. **Clones purlis has no record of** are adopted only through one ask, never automatically
   (Q184): "Trust these clones · Trust all / Choose…", listing each clone with its remote and when
   it was made. `purlis doctor` lists the same clones, and its "Move into repos/" fix offers the
   trust in the same step.

### 17. Phases (Q112)

Ledger: R-281 to R-286, R-327.

1. **Phase 1:** the fixes; the baseline fix (Q114) and the setup row (Q115); event-log kinds for
   blocks; guard denies reported, with the marker, the finish-row suffix and the Blocked log;
   `PermissionDenied` on Claude Code; "Blocked lately" folded in; the Journal tab; the loop rule;
   the app-performed merge hold; no window-only dead ends (decision 16); attendance (Q142), at the
   end of the phase.
2. **Phase 2:** app-performed pushes; the Sandbox view and the Allowed list; guard switches, with
   `OVERRIDE_NOTE` rewritten; the one Settings page and the change sheet; the project tier, with
   the one change protocol; the timed exception; teaching; the mark and the renames; All chats.
3. **Phase 3:** MCP launch approval, review and pinning; the built-in tools table; declared merge
   commands; **purlis decides** at Full sandbox, confirmed per person, per machine and per
   project, with a preview of what stops being checked.
4. **Phase 4:** the policy keys; MDM, with the strictest value per key; the keychain-held
   last-known-good copy; the ruleset Notice; the remote audit sink; the harness matrix as a
   release gate.

**What comes first.** The asks-scopes tickets 01, 02, 06 and 07 (#6340, #6341, #6345 and #6346;
spec #6339) land before the phase 1 UI slices that touch Asks, Updates or the right bar, and only
those (Q203, decided by the orchestrator, for the person's review). The fixes that come first
(Q26) are each on their own tested branch: #6315 (`cafbeec92`), the ✋ count of the sandbox consent
(`f5bb1e413`, already up as a pull request: Q204, decided by the orchestrator, for the person's
review), and #5242's secret-fill case (`f3dca8864`). The rest of Q26: run grants are counted on ✋,
and #5244 is fixed. #5244 is closed as completed, and the run-grant ✋ case that couldn't be
reproduced is a regression watch: phase 1's gate checks that no run-grant ask went uncounted in
the event log during the phase (Q199, decided by the orchestrator, for the person's review).

**How it is tested.** Where a test pins behaviour that already works, so it can't be seen red
first, its ticket shows it red by a deliberate mutation of the code, recorded in its report (Q208,
decided by the orchestrator, for the person's review). The matrix's test module is made from this
record's table by the first ticket that needs it, and this record goes up as its own pull request
before phase 1's gate (Q209, decided by the orchestrator, for the person's review).

## Consequences

### The harness matrix

Each layer, by harness, is **enforced** (purlis holds it outside the chat's reach), **unverified**
(it is reported from inside the chat's reach, so it is shown and never trusted to reset or
suppress a count) or **Can't tell** (the harness reports nothing purlis could read). **Every cell
gets a test**, which fails when the harness stops behaving as its cell says, and a cell changes
only with its test. A cell's value holds only as its test confirms it: the test module is built
from this table (Q209), and each cell then names its test, phase by phase. At No sandbox every
report on every harness is unverified (decision 5), so the table is for Basic and Full sandbox.

| Layer | Claude Code | Codex | opencode |
|---|---|---|---|
| Sandbox: files and egress | enforced (Bash in the sandbox; file tools by deny rules; web tools denied); confirmed by its test | enforced (the whole process, its hooks and MCP servers, in purlis's wrap); confirmed by its test | enforced (the whole process in purlis's wrap); confirmed by its test |
| A guard's block reported | enforced (the hook runs outside the chat's reach); confirmed by its test | unverified; confirmed by its test | unverified; confirmed by its test |
| The harness's own check's block reported | enforced (`PermissionDenied`); confirmed by its test | Can't tell (no denial signal yet; a hook-reported rejection becomes a block); confirmed by its test | Can't tell (no denial signal); confirmed by its test |
| The harness's prompts and asks reach Asks | enforced (the hook); confirmed by its test | Can't tell (rejected, never asked); confirmed by its test | enforced (held in the window); confirmed by its test |
| The mode the chat runs in (attendance) | enforced (the armed hook); confirmed by its test | Can't tell (bypass reported for every chat; attendance from how purlis started it); confirmed by its test | Can't tell; confirmed by its test |
| A hold the app performs (merge, push) | enforced; confirmed by its test | enforced; confirmed by its test | enforced; confirmed by its test |
| A non-purlis MCP tool's hold (phase 3) | enforced (the hook; the server runs outside the sandbox); confirmed by its test | unverified (the hook runs in the wrap); confirmed by its test | unverified (the hook runs in the wrap); confirmed by its test |
| A held built-in tool (phase 3) | enforced (the pinned fixture); confirmed by its test | Can't tell; confirmed by its test | Can't tell; confirmed by its test |
| Hosts recorded | enforced; confirmed by its test | enforced; confirmed by its test | enforced; confirmed by its test |
| The block loop's count | enforced; confirmed by its test | enforced for sandbox, policy and project blocks; unverified guard reports never count; confirmed by its test | enforced for sandbox, policy and project blocks; unverified guard reports never count; confirmed by its test |

`purlis policy check` prints this table against a policy (decision 15.6), and in phase 4 it is a
release gate.

### Where the new state lives (ADR 0069)

No store is added for what a chat did: the event log gains kinds. Each piece of state below has
its tier in `docs/plane-format.md`, marked *decided, not yet written*, and its ticket writes it
there as built.

| State | Tier |
|---|---|
| Blocks, holds, grants, loop pauses, protection changes, answers to asks | the event log's: Machine, device-bound |
| Protection changes kept past the event log's retention (an append-only history) | Machine, device-bound |
| Guard and hold switches, who decides, and each Use it or Keep mine, all confirmed in the window | Machine, device-bound, in the record of what the person confirmed (ADR 0096 decision 7) |
| Project recommends, requires, `exceptions`, declared merge commands, MCP entries and their sort | Plane, as keys of `charter.toml` |
| MCP launch approvals and each server's pinned tools list | Clone state, beside the shipped MCP approvals, which no chat writes |
| The host record and its first-seen counts | Machine, device-bound, in the network record |
| The blocked marker's "since you last looked" | Machine, device-bound, in the Inbox's record of what the person has seen |
| The last-known-good policy copy | Keyring |
| "Change for this chat only", a timed exception, one-use grants, the viewed SHA | none: app memory only (a chat-only change survives the chat's restart, not the app's) |

### Otherwise

- Guard A4 reads attendance, not the harness's mode, so a Full sandbox chat run by purlis
  deciding keeps the release floor off while the person attends it, and an unattended task keeps
  it on.
- Until declared merge commands (phase 3), `node tools/land.mjs` and any script like it pass
  purlis, and the merge hold sees only what the guards already read. The ruleset check (decision
  7.8) is the real lock meanwhile, and the Sandbox view says so.
- The classifier's 59 false positives end only where the person chooses purlis decides at Full
  sandbox (phase 3). Before it, every Claude Code chat keeps the harness's own check, and its
  blocks are seen and tagged instead of silent. They never count toward the block loop.
- The scenario specs find every block from its chat's marker and every hold from the ✋, and each
  hook give-up has a test of its own.

## What it amends

- **[ADR 0067](0067-a-chat-runs-in-a-sandbox-charter-compiles-for-its-harness.md)** (the sandbox):
  the level's badge becomes the mark (decision 12); the floor, Basic sandbox, is the default and
  the scaffold stops writing `mode = "on"` (decision 2.1); team mode becomes the project tier,
  recommends and requires, under one change protocol (decisions 3 and 4.6); hosts are recorded at
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
  what was confirmed, which no chat writes, now also holds guard switches, who decides and the
  answers to the project's settings.
- **[ADR 0119](0119-the-inbox-is-the-persons-own-in-three-views-on-the-right-bar.md)** (the right
  bar): the right bar gains the **Sandbox** view, read-mostly, with the shield icon
  (decision 12); its decision 10's first-open baseline never marks a loosening as seen
  (decision 4.6); Notices become searchable, filterable and recoverable once dismissed
  (decision 16).
- **[ADR 0121](0121-attention-what-waits-on-you-reaches-you-and-nothing-else-interrupts.md)** and
  its amendments (attention):
  - blocks are not in Asks; they are a marker at the "Just so you know" level (decision 9);
  - [budgets and loops](0121/2026-11-10-budgets-and-loops-pause-into-asks.md): looping gains the
    block loop on the same target, and denials the sandbox or a guard makes are now counted
    (decision 10);
  - [risk as built](0121/2026-11-10-risk-is-shown-as-built.md): merge is a risk class, a push maps
    to protected push and a destructive MCP tool to Delete (decision 7.7);
  - [consents and confirmations](0121/2026-11-10-consents-and-confirmations-are-asks.md): the
    sandbox consent is in Asks only while it can be answered (decision 9.5), and a refusal whose
    only way out is in the window is an ask (decision 16);
  - [keyboard triage](0121/2026-11-10-keyboard-triage-and-the-asks-keyboard-ruling.md) skips
    holds (decision 7.7).
- **[ADR 0112](0112-one-notice-for-one-need-a-person-may-allow-an-undeclared-program.md)**
  (grants): a one-use grant exists only for MCP tools and held built-in tools; merges and held
  pushes are performed by the app and never granted (decision 7.6).
- **[ADR 0093](0093-a-blocked-local-socket-or-system-service-is-asked-about-like-a-host.md)** (the
  consent): what a project expects is answered in one place (decision 9.5); a sandbox refusal is
  an ask only while the chat waits, and a block after (decision 9.3).
- **[ADR 0069](0069-every-store-charter-writes-is-in-one-of-four-tiers.md)** (tiers of stores):
  the new state is placed as [Consequences](#where-the-new-state-lives-adr-0069) says, and each
  piece names its tier in `docs/plane-format.md` before it ships.
- **[ADR 0076](0076-a-run-moves-only-by-a-named-cause-and-a-chats-state-is-read-from-its-runs.md)**
  (no parsing): the loop counts hook events and argument digests, and a block's `reason` is shown
  as written, never read and never stored (decisions 9.2 and 10.3). A loop pause is a named cause.
- **Toolgate's "no switch" ruling** (`toolgate::OVERRIDE_NOTE`: "there is deliberately no config
  key, environment variable or switch that lifts a purlis denial: one purlis could read is one a
  committed file could flip") is reversed. Its reason no longer holds: a guard switch counts only
  once confirmed in the window, through a record no chat writes (ADR 0096 decision 7), a commit
  can only tighten it, and a policy can lock it (decisions 2.2 and 4.6). `OVERRIDE_NOTE` is
  rewritten in phase 2.

## Not chosen

Each was agreed in an earlier round and later revised; the design notes' decision history keeps
it.

- **Presets** (Relaxed, Balanced, Strict, Standard, Custom; Q31, Q59, Q77). Dropped by Q101: a
  person sets the level, the guards and who decides. *Why:* a preset tied the level, the mode and
  the guards into one name, so changing one meant leaving the preset for Custom, and Strict had
  already been dropped (Q59) because no preset may promise what purlis doesn't enforce.
- **Copy command** on a block by the harness's own check ("Run it yourself", Q61). Dropped by
  Q139: the row says "purlis can't allow this; only {Harness}'s own check decides". *Why:* purlis
  can't allow what that check refused, and a command handed out to run by hand would take it past
  the only judge that refused it, which the honesty principle rules out.
- **Push grants** (Q73, Q99). Replaced by Q135: the app performs the push it shows, and grants
  exist only for MCP tools and held built-in tools. *Why:* the security round found forge
  controls that only read a command's words; a grant to the chat lets it run something other than
  what the person read, while a push the app performs is exactly the URL, ref and SHAs on the row.
- **"MDM wins"** over the policy file (Q110). Replaced by Q138: the strictest value per key wins.
  *Why:* if one source won outright, a looser MDM value would undo a stricter file value, and a
  policy only ever raises minimums (Q63, Q106).
- **A first-run picker** (Q32, Q64, Q78). Replaced by Q115: one Protection row at setup, with
  Basic sandbox preselected, which finishes in one press. *Why:* a picker put a decision in front
  of every person before their first chat; protection is chosen at setup and easy to change later
  (Q113), so setup stays one press.
- **Blocks in Asks**, as a Refused line (Q3). Revised by Q68: Asks holds asks only, and a block is
  a marker on its chat. *Why:* a block waits on no one, and Asks counts only what waits on the
  person (ADR 0121); in Asks, every guard or classifier refusal would have been noise beside the
  real asks.
- **The one-press merge without a viewed SHA** (Q97). Refined by Q136: one press only when the
  head is the SHA the person last viewed, or one with an approving review. *Why:* the head can
  move after the person looked, so one press could merge commits nobody saw.
- **An "asks every time" bin** for MCP tools (Q128). Replaced by Q174: Read · Write · Destructive,
  with an Always ask checkbox per tool. *Why:* a tool belongs in one bin by what it does, and
  whether it asks is a separate choice the person makes per tool.
- **Counting the harness's own check toward the block loop** where the harness has no fallback
  (Q133). Replaced by Q153: it never counts. *Why:* that check is outside purlis, and a harness
  that refuses again has its own way out; a count that depends on each harness's fallback is not
  one rule.
- **Sandbox blocks as Notices** (#1903, #1904, #1906). Replaced by Q196 (decided by the
  orchestrator, for the person's review): a sandbox refusal is an ask while the chat waits and a
  block after. *Why:* a Notice is for a standing fault, and a refusal is neither standing nor a
  fault of the window (Q68, Q131, Q154).
- **A folder-trust question in Asks** (Q200). Revised by Q205 (decided by the orchestrator, for
  the person's review): it shows as "Waiting in its terminal", or Can't tell (Q214). *Why:* the harness's
  folder-trust prompt can come before any hook loads, so purlis can't hear it to answer it.
- **The name "Record"** for the per-chat tab (Q130). Replaced by Q211: **Activity**. *Why:* it
  clashed with the shipped session record.
- **The name "Activity"** for the per-chat tab (Q211). Replaced by Q213 (decided by the
  orchestrator, for the person's review): **Journal**. *Why:* it clashed with the shipped
  Activity view of what a chat and its tasks said to each other.

## Amendments

An amendment to this decision is a file of its own in [`0124/`](0124/), named `<yyyy-mm-dd>-<slug>.md`,
never a section added to this record (#1982). The first expected is the person's review of the
decisions the orchestrator made, the Journal's name among them (decision 11.3).
