# 0126. One project policy governs every chat decision (Cedar)

Status: proposed (2026-11-16), a draft for review. It follows the person's rulings Q307, Q308b,
Q309 to Q319 and Q320, and the orchestrator's Q319a (O1–O4), Q320a (N1–N5) and Q320b
(R2-1–R2-4), in the who-judges orchestration (`work/who-judges/decisions.md` on the plane). It
also takes in the calls of the spec's first four Fable review rounds
(`work/who-judges/tickets/m75-policy/review-round-1.md` to `review-round-4.md` on the plane).
Q320a.N1 amends Q319 C6: with the seal opt-in on, the Linux and Windows seal gap is narrowed, not
closed, and stays declared. The rulings come from the wayfinder map "One project policy governs everything a chat can do" (#6969)
and its fifteen tickets (#6970 to #6981, #6986 to #6988).

It builds on ADR 0125 (blocks off by default and opt-in per family: Q308, Q308a and Q308b) and
keeps its names. The M75 spec (`work/who-judges/tickets/m75-policy/spec.md` on the plane) holds
the requirements; this record holds only the choices that are hard to reverse, and why.

## Context

purlis makes 1,199 decisions about a chat (the #6970 inventory, read from `main` at
`eb9c63fe8`): 819 of them hard-coded and 247 made in more than one place.
- Two different things are already called "policy": the project file's `[sandbox] policy` and the
  administrator's `/etc/purlis/policy.json`.
- The admin locks reach only the sandbox and dispatch.
- About 15 stores hold grants, nevers and remembered answers, most of them writable by an
  unsandboxed chat.
- The same rule behaves differently per harness (#6972).

ADR 0125 made every block a family switch (`off`, `log`, `enforce`) with templates. It is the
first slice. This record makes the policy the one place every decision is stated, judged and
explained.

## The decision

### 1. One policy, in Cedar

**Policy** is the project's complete, committed statement of what a chat may run, act on or
change, and when, by whom and where (Q307 Q2, Q3). Every decision purlis makes about a chat is in
it: the sandbox, the network, ports and processes, guards, asks and consents, dispatch and
personas, vaults and secrets, the browser, skills, tools and the harness permission mode.

It is written in **Cedar** (`cedar-policy`, pinned to an exact 4.x), parsed and evaluated only in
the session host (Q311 point 1, Q309 point 3).

- Cedar evaluates deterministically and independently of rule order, and that is proved in Lean.
  Forbid beats permit, so a higher level's forbid is a guardrail no lower permit lifts. Each
  answer names the rules that decided it. Its strict validator rejects a rule that names
  something the schema doesn't know, and its symbolic analyser (SymCC) can prove invariants and
  locks (#6971).
- The principal is always a **Chat**, in its Workspace, Persona, Harness, Runner and Plane. The
  person is never a principal (Q311 point 2).
- An **ask** is a forbid carrying `@ask("you")`. The outcome is ask when every deciding forbid
  asks, deny when any doesn't, and allow otherwise, so deny > ask > allow follows from Cedar's own
  forbid-beats-permit (Q311 point 6). An ask with no permit beneath it is a deny (Q318 point 2).
- Every rule carries an `@id`, unique across all levels, and every outcome names the rule that
  decided it. The shipped default allows by default and ends in the catch-all permit
  `everything-else`, so even a default deny is named (Q311 points 1 and 7, Q316 point 1, Q318
  point 1; review round 1, S-B5).
- Each `[policy]` family is a Cedar action group of the same name, and its mode (`off`, `log`,
  `enforce`) is applied after Cedar decides (Q311 point 8). There are 25: ADR 0125's 24 and
  `audit`, which holds limits only and takes no `off` or `log` (Q313 point 4).
- A family's mode acts on that family's forbids and asks only. Its permits always count, so
  turning a family off never denies. A permit stops counting only when `[policy.rules]` names it
  `off`, as deny-by-default's `"everything-else" = "off"` does. A permit takes `off` or `enforce`
  only, never `log` (Q319 C2, amending Q311 point 8; Q318 point 1; Q319a O2).
- An `@ask` forbid on `read`, `write`, `connect` or `dial` (the socket action) takes the `asks`
  family's mode. The action stays in its own family for everything else (Q319 C4, Q319a O3).
- **The escape denies are not in any family.** The process and service denies that keep a chat
  inside its OS wrap belong to the sandbox level (`floor` or `full`), so no family's mode, and so
  no template's "everything logs", lifts them (review round 1, S-B2). They are not among the
  fixed things of §5 either: they are a separate step that applies whenever the level is `floor`
  or `full` (review round 2, F2-3).
- **The `sandbox` family has one switch, its level.** `[policy] sandbox = off|floor|full` is its
  only key, with no `off`/`log`/`enforce` of its own; `open` stays "everything off" (review round
  2, F2-1; Q320b R2-2). What the hooks do at each level is in the spec.

**Rejected.** OPA/Rego through regorus: pre-1.0, with no allow/deny conflict model, no equivalence
or reachability analysis, and an unfinished explanation API. A TOML evaluator of purlis's own was
the fastest (0.5 µs against Cedar's 94 µs p50), but every guarantee above would be custom code,
against CLAUDE.md's second priority (#6971).

**Hard to reverse because** rules, the schema, the shipped default, the tests' oracle and every
refusal's wording name Cedar's entity types, actions and `@id`s. Leaving Cedar would mean
rewriting every project's `policy.cedar`.

### 2. The policy in force

- The project's rules live in one committed `policy.cedar` beside the project file at the plane
  root. They are read as committed at the plane root's HEAD, never from a branch folder and never
  from the working file (Q309 point 1). With no HEAD there is no committed policy: the accepted
  copy, or else the shipped default, applies (Q320a N3).
- The text the person last accepted on this machine is kept **sealed**. Every decision is the
  stricter (deny > ask > allow) of the committed policy and the accepted policy, and "stricter"
  covers all the policy's data: scopes, modes, limits, locks and the scan-allow data. Named sets
  and tool declarations are never merged across the two: each evaluation uses its own side's, and
  a tool whose command differs runs the accepted one. So a tightening applies at once, and a
  loosening waits for the person's **loosening press**. The shipped default stands in until the
  first acceptance (Q309 point 2; review round 1, S-B8; review round 2, S2-SF1).
- **Tightenings are sealed into the accepted floor.** The app seals every tightening it sees
  into the **accepted floor** (distinct from the sandbox level `floor`), which is the conjunction
  of every committed version seen since the last press. A sealed, monotonic manifest covers the
  whole sealed store, and only the entries it lists are loaded: a sealed file it doesn't list is
  ignored, with a Notice. Moving HEAD back, committing a broken file, deleting the store's
  accepted entry, or replaying an older store or a removed grant then loosens nothing (review
  round 1, S-B1; review round 2, S2-SF2; review round 4, SEC4-SF2, F4 SF-2).
- **The anchors live outside the state folder,** in an app-only keyring item (the Keychain on
  macOS) that purlis's one-item writer refuses, keyed by the plane's real path, never the clone
  key. With the anchor present, a missing or older manifest, or a sealed entry other than a grant
  that is missing or fails, is the **deny state**: the chat still starts, every action but
  `start` is refused, and nothing is allowed until the person re-presses (review round 2, S2-B1,
  S2-B2, F2-2; Q320b R2-3; review round 3, SEC3-B1, SEC3-B2; review round 4, F4 nit).
- **A plane has a stable id, and the machine keeps a registry of anchored planes.** The plane id
  is kept in the plane's sealed manifest. The keyring item holds a sealed, machine-level registry
  of each anchored plane's id, real path and the path as opened. A plane whose id is anchored but
  is seen at another path, an anchored path with no matching manifest, and a sealed store with no
  anchor are each the deny state, so moving a plane never reads as a fresh install. Only no anchor
  and no store is a fresh install, where the shipped default applies. The sandbox holds the plane
  root against rename where it can; where it can't, that is a declared gap (review round 4,
  SEC4-B1).
- **The Everywhere nevers have their own manifest, anchor and counter** in the same keyring item.
  If that manifest is missing or older, every plane is in the deny state (review round 4,
  SEC4-B2). On Linux and Windows every anchor is only as strong as the declared seal gap (Q320b
  R2-4).
- Only the session host loads, verifies and evaluates the policy. Hooks, the proxy and dispatch
  ask the host, and an action is refused if the host can't be reached (Q309 point 3). This
  replaces ADR 0125 §3's `$PURLIS_POLICY` hand-off. The host is found from the plane, never from
  the environment, and the client requires the listener to be the app that started the chat. With
  no host, a chat inside a plane is refused; a process outside any plane runs the shipped default
  under `strict`, locally, with the Organisation's files layered on it. A process counts as
  outside any plane only when its working directory, its target's real path and its ancestry are
  all outside every anchored plane; otherwise it asks that plane's host, or is refused (review
  round 1, S-B3; review round 2, S2-B3; review round 3, SEC3-SF3; review round 4, SEC4-SF1).
- A team changes the policy through git. Anyone, a chat included, may propose a change in a
  branch. Each machine's window shows the rule-by-rule diff, the commit and its author, and
  accepts it with one press. purlis enforces no commit review (Q309 points 5 and 6, Q313
  point 2).

**Hard to reverse because** the seal, the press, "stricter of two" and the accepted floor are what make
Q307 Q4 hold: **a chat can never change the policy that governs it**. It holds fully on macOS.
On Linux and Windows it holds with a declared gap: the full gap by default, and with the seal
opt-in on a narrowed gap, since a same-user process can still read the app's memory or start a
second instance (§7; Q319 C6 as amended by Q320a N1). Any later design must keep that.

### 3. Levels and locks

- Five **levels**, top to bottom: **Organisation** (an admin-owned folder per OS, delivered by
  MDM and never fetched), **Project**, **Workspace**, **Persona** and **You**. A chat is not a
  level (Q310 points 1, 2 and 6).
- The Organisation's folder holds two files: `policy.cedar` for its rules, and `policy.toml` in
  the full syntax a project uses, `[policy.rules]` and `[policy.sets]` included, so an
  Organisation can go deny-by-default. The values are typed and validated, and the Cedar files
  stay pure rules for the validator and SymCC (Q319 C5, Q319a O1). The folder's owner and ACL are
  verified at load on every OS; a folder that fails reads as `strict` plus every lock (review
  round 1, S-B7).
- Workspace and Persona defaults are written in the project's committed file, as
  `[policy.workspace.<w>]` and `[policy.persona.<p>]`. Workspaces have no files of their own, and
  persona.md holds no permissions (Q319 C3, Q309 point 1, Q315 point 3).
- For defaults (the template and family modes), the nearest level wins, and the stricter wins
  when a workspace and a persona both set one. For rules, deny wins at every level (Q310 point 3).
  A rule mode and a same-`@id` replacement reach only rules at their own level or below (review
  round 1, S-B5).
- Organisation loosens without a press. Project, Workspace and Persona loosen only after it, and
  You only tightens. A level writes rules only about its own chats (Q310 point 4).
- Organisation and Project may **lock** a family, an action, a grant scope or timed grants. A
  lock freezes the default below it and drops a lower permit on the locked key, with a Notice.
  Tightening is never locked (Q310 point 5, Q314 point 10). Locks replace "admin locks".
- A broken file is never skipped. A broken Project, Workspace or Persona file falls back to the
  copy the person last accepted, else `strict`. A broken Organisation file reads as `strict` plus
  every lock. The start is not refused (Q310 point 7, Q309 point 7).

**Hard to reverse because** every admin's delivered files, every project's `[policy.locks]` and
every refusal's "Locked by <owner>" are written against these five levels and their precedence.
Adding, merging or reordering a level later would change what an existing lock binds on every
machine that holds one (review round 3, F3 nit).

### 4. Grants are the person's sealed answers

The policy decides allow, ask or deny. A **grant** is the person's answer to an ask, kept sealed
on this machine. It is used only where the policy says ask, and never lifts a deny, a lock, a
never, the kill switch or a grader bundle (Q308b, Q312 point 2, Q314 point 11).

- Precedence: lock > the person's never > policy deny > policy allow > sealed grant > policy ask
  (Q308b; its "same persona" step is a rule of the shipped default).
- A grant covers one rule `@id`, one action, one resource and the context it was given under.
  When several asks decide one action, each needs its own grant (Q314 point 11; review round 1,
  S-B6 and S6).
- Every ask offers **Never**, a sealed rule of the person's own that holds in every mode, `open`
  included (Q314 point 12, Q308b).
- The four answer files become **one sealed answer store**, on this machine (Q315 point 5). The
  person's existing nevers are imported into it once and sealed; grants are not (Q320).

**Hard to reverse because** a grant's meaning is written into every sealed answer the person
gives.

### 5. What stays outside the policy

Only three things (Q308b, Q314 point 16):

1. **A chat can't change its own policy.** This includes that a chat never answers an ask or
   makes a grant, and that a seal request from a chat is refused. It holds fully on macOS; on
   Linux and Windows it holds with the declared seal gap, narrowed but not closed by the opt-in
   (§7; Q320a N1).
2. The **kill switch**, the person's emergency stop.
3. The **grader bundles**: a skill can't grade itself.

Everything else is a rule, a limit, a sealed answer or data, and a project may loosen anything
(Q307 Q4). The escape denies are not a fourth fixed thing; they belong to the sandbox level (§1).

Two rules follow from the first, and are stated here so they aren't lost:
- **A dispatched chat is never looser than its asker:** each decision is also made for every
  asker up its chain, with that asker's nevers and You-level rules, and the strictest wins
  (Q308a, Q308b; review round 1, S7; review round 2).
- **No HEAD, no committed policy:** a plane root with no HEAD runs its accepted copy or the
  shipped default, and its working file is never read as committed (Q320a N3).

**Hard to reverse because** "a project may loosen anything" (Q307 Q4) is what lets the shipped
default allow by default and the templates be mode tables. Adding a fourth fixed thing later would
take a power from every project that relies on loosening it, with no rule to show for it (review
round 3, F3 nit).

### 6. A clean cut

Every setting becomes a rule (`policy.cedar`), a limit (`[policy.limits]`), a sealed answer or
data. It stays a plain setting only if it decides nothing a chat may do (Q315 point 1).
- `[sandbox]` is gone, and `persona.md` holds no permissions.
- There is no converter, and no release reads both the old keys and the new (Q315 point 11).
- A file that still holds a moved key reads as broken, with a Notice naming the key, its line and
  its new home. The old grant files are not carried over (Q315 points 2, 3, 5, 11 and 12; Q320).
- Only the Organisation's `/etc/purlis/policy.json` is still read, translated in memory (Q310
  point 6, Q315 point 6).

**Hard to reverse because** once released, there is no reader for the old keys.

### 7. Known gaps are declared, and the chat still starts

A key a harness can't enforce is a **known gap**. It is declared in Settings and doctor, and the
chat still starts. A project that wants a refusal writes a rule on `start` that refuses it while
a gap affects a family it enforces; the shipped default has none (Q319 C1, amending Q307 Q5's
"the start is refused"). An enforced action that has no point failing closed without its hook (a
host-brokered credential, the sandbox or the proxy) is such a gap too (review round 1, S-B4).

The seal is one such gap. On Linux and Windows no standard store binds a key to an unpackaged
app (#6986), so by default the seal stops hand edits and sandboxed chats only (on Windows, which
has no purlis sandbox yet, hand edits only), and the gap is declared. Two opt-ins **narrow** it:
"Confirm with Windows Hello each time purlis starts" on Windows, preferring a non-exportable
Hello signing key, and "Ask for a password each time purlis starts" on Linux (Q319 C6). They
don't close it: a same-user process can still read the app's memory or start a second instance,
so with an opt-in on the gap stays declared, as a narrowed gap (Q320a N1, N4).

**Hard to reverse because** the gap cells, Settings and doctor all assert "declared, and started":
refusing by default later would stop chats that start today, on every project that hasn't written
the rule.

## What it amends

- **ADR 0125:**
  - §1's "`[policy]` has no `sandbox` key": `[policy] sandbox = off|floor|full` replaces
    `[sandbox] mode` (Q315 point 2, Q318 point 9);
  - §3's `$PURLIS_POLICY`: the host evaluates (Q309 point 3);
  - its enforcement log, which is folded into the event log with no store of its own (Q313
    point 3).

  Each lands as an amendment file under `docs/adr/0125/`.
- **ADR 0124:** §3's "Project recommends" row and item 3, the wording of §4.1 and §4.6, and §9's
  "timed exception where it exists" (Q312 point 3), as an amendment file under `docs/adr/0124/`.
- **Rulings rather than records:** Q307 Q5's "the start is refused" (Q319 C1); Q311 point 8's
  family `off` "skips the family's rules" (Q319 C2); Q315 point 5's "nothing is carried over",
  for nevers (Q320); Q309 point 4's "counts as no acceptance", which under a current manifest
  now holds for a grant only, any other missing or failing sealed entry being the deny state
  (Q320b R2-3; review round 3, SEC3-B2; added by review round 4, F4 nit); and Q319 C6, as
  amended by Q320a N1 (with an opt-in on, the seal gap is narrowed, not closed, and stays
  declared). ADR 0126 states the amended reading.
- **CONTEXT.md:** "Policy" widens to the whole statement; "admin lock" becomes **Lock**; **Level**
  gains Organisation and You, with You at the bottom and tighten-only (the M75 spec lists every
  entry).

## Consequences

- A refusal, an ask and an allow each name a rule `@id`, its level and its owner, and
  `purlis policy why` answers for one action without running it.
- Enforcement is proved equal to the policy against an independent Cedar oracle, and each known
  harness gap is asserted as declared (Q317, Q319 C1).
- On Linux and Windows an unsandboxed program running as the same user can read the seal key
  unless the person turns on the seal opt-in, and even with it on can still read the app's
  memory. That is a declared gap, shown in Settings and doctor, full by default and narrowed with
  the opt-in (Q309 point 4, Q319 C6, Q320a N1, #6986). purlis has no Windows sandbox yet (M46,
  #565), so by default the Windows seal stops hand edits only (Q320a N4).
- The sandbox's paths are compiled from the same policy the hooks read, behind one function
  (#6988). How it compiles them is the spec's, and still proposed (review round 2, F2-4; trimmed
  by review round 3, F3 nit).
- Every existing settings file with a moved key reads as broken until a person rewrites it.
