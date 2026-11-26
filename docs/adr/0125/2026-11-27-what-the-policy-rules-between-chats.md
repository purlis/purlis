# Amendment 2026-11-27: what the policy rules between chats

Amends [ADR 0125](../0125-blocks-are-off-by-default-and-opt-in-per-family.md), which owns the
families and their modes, with the person's rulings of 2026-11-27 on "every chat-to-chat
interaction, and every enforcement, protection and rule on it, is controllable by the project
policy", after four Fable critiques of that idea. It binds the M75 record of the Cedar policy
("One project policy governs every chat decision", on the `m75-policy` integration branch, whose
number there collides with main's 0127 and is settled when it lands) and the M75 spec
(`work/who-judges/tickets/m75-policy/spec.md` on the plane) wherever they speak of chats.

## Where purlis was

- A project's policy switches very little of what one chat does to another. Most of it is
  hard-coded (messages only along the lineage, cancel and close only of one's own tasks, a
  browser closed only by its chat), or filed under a family whose choke point never asks
  (`docs/policy-inventory-families.md`, "unwired").
- In the M75 schema a `Chat` is only ever the principal. No action can name another chat, a
  chat's own branch or a sibling, and `Branch` and `Path` have no owning chat.
- Some of it is unchecked: any chat reads any session record and any branch folder of its
  workspace.
- One switch runs backwards. The forge route's NotYourBranch raises "Let this chat push it" only
  while `consents` enforces; under `off` or `log` no ask is raised and the push dead-ends, so
  turning the family down makes purlis stricter.
- `[policy] lineage` (`enforcement::LINEAGE_KEY`), a parent taking up its child's branch, maps to
  no M75 action.

## The decision

### 1. M75 takes the one-way doors, and M80 takes the rest

M75 decides only what cannot change after `// purlis-policy 1` ships: the fixed floor (2), the
permit-required class (3), the family names (4), the relation facts and their source (5) and the
schema's growth rule (6). It also maps today's chat-to-chat rows to rules or to `later:` targets.
A new milestone, **M80 "Chats as resources"** (milestone 59; the ruling called it M76, a number
the forge's "One login: your sign-in, brokered" already held), adds the `Chat` resource and the
actions between chats. Nothing is dropped: every gap the 2026-11-26 map lists is an M80 issue, or
an M75 issue where it is a bypass on `main` today. Its issues are #7753 to #7767 and #7770 to
#7774, its gate #7774.

### 2. The fixed floor

No level changes these, the Organisation included, and no template or mode reaches them. All
seven are the person's rulings of 2026-11-27; the last three additions (a browser the person
drives, no force push onto another chat's branch, no lateral message across personas or trees) were ruled
that day after the critiques. They join spec §2.5's fixed things (the kill switch, "a chat cannot
change its own policy", the grader bundles), and slice 08's map takes each as a `fixed:` target:

1. A chat cannot change, accept or seal policy, answer an ask or make a grant. **A chat's message
   is never the person's word**, whoever relays it.
2. The person's nevers, Keep blocked, never-above and the kill switch hold in every mode.
3. A chat the person typed into cannot be told, cancelled or closed by any other chat. A browser
   the person drives cannot be closed by a chat.
4. No chat uses another chat's browser session, or a vault of a persona other than its own.
5. Every act across chats is audited with both chat ids, in every mode, `open` included. This is
   an `audit` record, which takes no `off` or `log` (Q313.4).
6. A review or an approval binds to a commit sha, and covers only that sha: any later push is held
   again, and a push by anyone but the branch's owner voids it. There is no force push onto
   another chat's branch.
7. No chat messages a chat of another persona or another tree laterally. A message between a chat
   and its own task or asker is not this item's, whatever the persona.

A rule can tighten any of them. A rule that would loosen one is refused at load, as a rule
naming an escape deny is (spec §3.5).

### 3. New abilities between chats are permit-required

An action that lets a chat do something to another chat that it cannot do today (message a
sibling, observe another lineage, act on another chat's branch or browser) is in the
**permit-required class**:

- it is allowed only by an explicit `permit` that names it. `policy-everything-else` does not
  reach it, and its no-permit deny is not a forbid, so no family mode lifts it;
- no template turns it on, and neither does `watch` or `log`. `open` leaves it off too;
- a wildcard permit (one that names no action, or the whole family) does not reach it: only a
  permit naming the action does;
- **an Organisation permit for it is still a loosening, and waits for the person's press on this
  machine.** This is an explicit exception to spec §4.3 and Q309.8, under which an Organisation
  rule applies without a local press: for this class only, it is a loosening like any other (ADR
  0125 §3). The press is sealed in the accepted floor (the sealed answer store, M75-07a), keyed by
  the permit's `@id` and the Organisation file's digest, so a changed Organisation file needs the
  press again (#7754).

Each M80 action names whether it is in the class. An action in it can leave the class only by a
later amendment, never by a release.

Reaching another chat's port is not in the class: `reach` decides it, with `context.owner` (Q338).

### 4. The `dispatch` family covers `dispatch-to` only, and `chats` is reserved

- `[policy] dispatch`'s mode governs `dispatch-to` (a dispatch or a handoff) and nothing else.
  `dispatch = "off"` opens no message, cancel, close or read between chats.
- The family name **`chats`** is reserved for M80's actions between chats. No action, set or
  `@id` prefix outside it may take the name, and slice 01's validation refuses one that does.
- `chats` has no upward action: a task's report and question to its own asker are protocol (7),
  and `report` stays a `consents` action. Its first actions go down the lineage: telling one's
  own task and answering its question (#7755).

### 5. Ownership and lineage are facts the host supplies

- The relation is **computed by the host once, for the real actor, and carried in context**,
  never derived in Cedar from entity attributes:
  - `context.owner`, on an action whose resource a chat owns: `"self"` (the actor), `"task"` (a
    chat down the actor's own dispatch chain, at any depth), `"asker"` (a chat up it, at any
    depth), `"other"` (any other chat, a sibling or cousin included) or `"none"` (no chat: the
    person's, or not recorded). There is no `context.lineage`: the values say the direction, and a
    rule that means only its own tasks reads `"task"` alone;
  - `context.relation`, on an action whose resource is a Chat (M80): the target's relation to the
    actor. The values are fixed in M80's spec; `"self"`, `"asker"`, `"task"`, `"sibling"` (a task
    of the actor's own parent, and only that: never a cousin) and `"other"` are proposed.
- **Why not in Cedar.** Spec §6.5 judges a dispatched chat's act again for every asker up its
  chain, in the child's context. A relation Cedar derived from entities would be recomputed for
  each asker. A child's act on its sibling's branch would then read `"task"` in the parent's
  evaluation, since the sibling is the parent's task. The relation is a fact about the act and
  its real actor, so each asker's evaluation reads the same value the child's did. **Lineage is
  the actor's own line, up and down. A sibling or cousin, reached only through a common ancestor,
  is never lineage**, and no evaluation makes one so.
- **Rejected:** a `Chat in Chat` hierarchy (every `principal in` would match descendants, and the
  tree changes while a chat runs); a `Branch.owner` attribute (an owner is relative to the actor,
  and a stored attribute would be read from a store a rule could be pointed at); and a
  `sibling_of` relation (the same laundering, one edge wide).
- **Two ownerships, kept apart.** A **forge branch** (`Branch`) is owned by the chat the forge
  route recorded creating it (`branch_owner`). A **local branch folder** (`Path`) is owned by the
  chat that holds its claim (`piececlaim`). `push` reads the first; `commit`, `update` and
  `discard` read the second.
- **A write in a claimed branch folder is its owner's only** (Q338). A write there (a file write,
  `commit`, `update` or `discard`) by a chat whose `context.owner` is not `"self"` is allowed only
  by a permit that names it (3). The one-writer mutex (7) still holds.
- Both are **host-supplied** in the schema's source table (spec §3.2), from the host's own
  dispatch records, forge-route records and claims. A chat never writes them (#7748).
- **Ownership moves only by hand-over or the person's press.** A finished task's report, or its
  `worktree done`, hands its branch, forge record and folder claim to the chat that dispatched it,
  where `guards-git-take-up-child` permits it (it reads only `"task"`, and replaces `[policy]
  lineage`). For an unfinished task it decides nothing. Anyone else asks the person, through "Let
  this chat push it".

### 6. The schema grows only additively within a version

Before version 1 ships: within `// purlis-policy 1` the schema may add entity types, actions,
optional attributes and optional context fields. It never renames, removes or retypes one, never
makes an optional field required, and never moves an action between families.

- A new action that is not in the permit-required class carries the decision purlis already made
  as a rule of the shipped default. Adding it never changes an answer.
- A file valid under one version-1 schema stays valid under every later one.
- A file that names what an older purlis's schema lacks is a broken file there (spec §4.6), as any
  file that fails strict validation is.
- The schema is committed as a snapshot, and CI fails on a change that is not an addition (#7769).
- Anything else is version 2, with a migration.

### 7. Protocol stays not a chat decision

These are protocol: they have no action, keep working in every state, the deny state of a broken
acceptance included, and no rule names them:

- a chat stopping its own task;
- a task reporting and asking up to its own asker;
- routing a task's question to its asker and on to the person (Dispatch #86);
- the one-writer folder mutex (a claimed folder takes one writer, outside `guards.git`'s mode).

### 8. Reads between chats

- **Session records** stay readable by every chat of the same workspace, always quoted as data,
  never as instructions.
- **Another chat's branch folder**, and its screen or transcript, are readable by its own lineage
  only. For a branch folder this is a **ruled tightening**: today a read has no check, so the
  outcome moves on purpose. Its mechanism is a forbid of the shipped default
  (`chats-folder-read-other-lineage`, on a read in a claimed folder whose owner is `"other"`),
  compiled into the OS sandbox's read denies for those folders and into the file tools' deny
  rules, so both refuse the same paths (#7758).
- **Merging another lineage's pull request** keeps today's outcome as a default rule, which a
  project may forbid by owner; it is no new ability, so it is not permit-required (#7759).
- A policy may tighten any of these further. A read beyond them is permit-required.

### 9. Messages between siblings (M80)

- Allowed only between **siblings: tasks dispatched by the same chat** (the same parent only, never
  a cousin), in the same workspace and with the same persona. Their relation is `"sibling"`.
  Laterally across personas or trees: never, which is floor item 7.
- It has its own design and Fable review before it is built (#7770).
- A message is data, shown on the lineage's Activity.
- Messages are rate-capped per parent, not only per pair.
- A wait cycle (A waits on B, which waits on A) raises one Blocking ask at once.

### 10. The push dead end is fixed on `main` now

Wherever a NotYourBranch refusal stands, "Let this chat push it" is raised in every `consents`
mode (#7749). A family turned down never makes purlis stricter. In M75 the NotYourBranch push goes
through the same `push` decision as a brokered and a plain push (#7039, #7747).

**A push whose `context.owner` is not `"self"` is outside `policy-everything-else`** (`"none"`
keeps today's outcome). It is a no-permit deny, not a forbid, so no family mode lifts it, `open`
and `watch` included; `guards-git-push-not-own` asks, and the only way out is the window's press
(#7034, `a_push_to_another_chats_branch_is_refused_under_open_and_watch_until_the_press`).

## Consequences

- M75 maps `[policy] lineage` and NotYourBranch, its two unmapped chat-to-chat rows, to rules:
  `guards-git-take-up-child` and the `push` decision.
- The inventory map takes a `later:<issue>` target for a row an M80 issue owns. The M75 gate
  accepts one only with the person's approval by name, checked against a committed approvals file
  and a committed M80 issue list, never the live forge (#7746). A row with two targets is split
  into sub-rows (Dispatch #82a, #82b, …).
- M75's docs carry a table of what the policy does not yet control between chats, each row
  naming its M80 issue.
- Until M80, a chat-to-chat row that is hard-coded today stays hard-coded: a template that turns
  families off changes none of it.
