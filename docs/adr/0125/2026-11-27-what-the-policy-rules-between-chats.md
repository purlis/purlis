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

### 1. M75 takes the one-way doors, and M76 takes the rest

M75 decides only what cannot change after `// purlis-policy 1` ships: the fixed floor (2), the
permit-required class (3), the family names (4), the relation facts and their source (5) and the
schema's growth rule (6). It also maps today's chat-to-chat rows to rules or to `later:` targets.
A new milestone, **M76 "Chats as resources"**, adds the `Chat` resource and the actions between
chats. Nothing is dropped: every gap the 2026-11-26 map lists is an M76 issue, or an M75 issue
where it is a bypass on `main` today. "M76" is the ruling's name for it. The forge's milestone
M76 is already "One login: your sign-in, brokered", so the new milestone's number is settled when
it is created. Its issues are #7753 to #7767.

### 2. The fixed floor

No level changes these, the Organisation included, and no template or mode reaches them. They
join spec §2.5's fixed things (the kill switch, "a chat cannot change its own policy", the grader
bundles):

1. A chat cannot change, accept or seal policy, answer an ask or make a grant. **A chat's message
   is never the person's word**, whoever relays it.
2. The person's nevers, Keep blocked, never-above and the kill switch hold in every mode.
3. A chat the person typed into cannot be told, cancelled or closed by any other chat. A browser
   the person drives cannot be closed by a chat.
4. No chat uses another chat's browser session, or a vault of a persona other than its own.
5. Every act across chats is audited with both chat ids, in every mode, `open` included. This is
   an `audit` record, which takes no `off` or `log` (Q313.4).
6. A review or an approval binds to a commit sha. A push by anyone but the branch's owner voids
   it. There is no force push onto another chat's branch.

A rule can tighten any of them. A rule that would loosen one is refused at load, as a rule
naming an escape deny is (spec §3.5).

### 3. New abilities between chats are permit-required

An action that lets a chat do something to another chat that it cannot do today (message a
sibling, observe another lineage, act on another chat's branch, port or browser) is in the
**permit-required class**:

- it is allowed only by an explicit `permit` that names it. `policy-everything-else` does not
  reach it, and its no-permit deny is not a forbid, so no family mode lifts it;
- no template turns it on, and neither does `watch` or `log`. `open` leaves it off too;
- an Organisation permit for it is still a loosening, and waits for the person's press on this
  machine, as every loosening does (ADR 0125 section 3).

Each M76 action names whether it is in the class. An action in it can leave the class only by a
later amendment, never by a release.

### 4. The `dispatch` family covers `dispatch-to` only, and `chats` is reserved

- `[policy] dispatch`'s mode governs `dispatch-to` (a dispatch or a handoff) and nothing else.
  `dispatch = "off"` opens no message, cancel, close or read between chats.
- The family name **`chats`** is reserved for M76's actions between chats. No action, set or
  `@id` prefix outside it may take the name, and slice 01's validation refuses one that does.
- `report` is already a `consents` action, so a chat's report to its asker is not named `report`
  in `chats`. M76's spec names it (`report-up` is proposed).

### 5. Ownership and lineage are facts the host supplies

- The relation is **computed by the host once, for the real actor, and carried in context**,
  never derived in Cedar from entity attributes:
  - `context.owner`, on an action whose resource a chat owns: `"self"` (the actor),
    `"lineage"` (an asker up the actor's chain or a task down it), `"other"` (any other chat) or
    `"none"` (no chat: the person's, or not recorded);
  - `context.relation`, on an action whose resource is a Chat (M76): the target's relation to the
    actor. The values are fixed in M76's spec; `"self"`, `"asker"`, `"task"`, `"ancestor"`,
    `"descendant"`, `"tree"` (the same person-started tree, neither line) and `"other"` are
    proposed.
- **Why not in Cedar.** Spec §6.5 judges a dispatched chat's act again for every asker up its
  chain, in the child's context. A relation Cedar derived from entities would be recomputed for
  each asker. A child's act on its sibling's branch would then read `"lineage"` in the parent's
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
- Both are **host-supplied** in the schema's source table (spec §3.2), from the host's own
  dispatch records, forge-route records and claims. A chat never writes them (M75 issue
  "Host-held owner and lineage facts a chat cannot forge").
- **Ownership moves one way** (ruling 6). A finished task's report, or its `worktree done`, hands
  its branch, forge record and folder claim to the chat that dispatched it. Anyone else asks the
  person, through "Let this chat push it".

### 6. The schema grows only additively within a version

Before version 1 ships: within `// purlis-policy 1` the schema may add entity types, actions,
optional attributes and optional context fields. It never renames, removes or retypes one, never
makes an optional field required, and never moves an action between families.

- A new action that is not in the permit-required class carries the decision purlis already made
  as a rule of the shipped default. Adding it never changes an answer.
- A file valid under one version-1 schema stays valid under every later one.
- A file that names what an older purlis's schema lacks fails strict validation there, which is a
  deny (spec §6.3).
- Anything else is version 2, with a migration.

### 7. Protocol stays not a chat decision

These keep working in every state, the deny state of a broken acceptance included, and no rule
names them:

- a chat stopping its own task;
- reporting and asking to its own asker;
- routing a task's question to its asker and on to the person (Dispatch #86);
- the one-writer folder mutex (a claimed folder takes one writer, outside `guards.git`'s mode).

### 8. Reads between chats

- **Session records** stay readable by every chat of the same workspace, always quoted as data,
  never as instructions.
- **Another chat's branch folder**, and its screen or transcript, are readable by its own lineage
  only.
- A policy may tighten either further. A read beyond them is permit-required.

### 9. Messages between siblings (M76)

- Allowed only within the same person-started tree, the same workspace and the same persona.
  Across personas or trees: never, which is floor-level.
- A message is data, shown on the lineage's Activity.
- Messages are rate-capped per tree, not only per pair.
- A wait cycle (A waits on B, which waits on A) raises one Blocking ask at once.

### 10. The push dead end is fixed on `main` now

Wherever a NotYourBranch refusal stands, "Let this chat push it" is raised in every `consents`
mode. A family turned down never makes purlis stricter. In M75 the NotYourBranch push goes
through the same `push` decision as a brokered and a plain push.

## Consequences

- M75 maps `[policy] lineage` and NotYourBranch, its two unmapped chat-to-chat rows, to rules:
  `guards-git-take-up-child` and the `push` decision.
- The inventory map takes a `later:<issue>` target for a row an M76 issue owns. The M75 gate
  accepts one only with the person's approval by name.
- M75's docs carry a table of what the policy does not yet control between chats, each row
  naming its M76 issue.
- Until M76, a chat-to-chat row that is hard-coded today stays hard-coded: a template that turns
  families off changes none of it.
