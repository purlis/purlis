# 0126. One project policy governs every chat decision (Cedar)

Status: proposed (2026-11-16), a draft for review. The person's rulings Q307, Q308b and Q309 to
Q319 in the who-judges orchestration (`work/who-judges/decisions.md` on the plane), from the
wayfinder map "One project policy governs everything a chat can do" (#6969) and its fifteen
tickets (#6970 to #6981, #6986 to #6988). It builds on ADR 0125 (blocks off by default and opt-in
per family, Q308, Q308a and Q308b) and keeps its names. The M75 spec
(`work/who-judges/tickets/m75-policy/spec.md` on the plane) holds the requirements; this record
holds the choices that are hard to reverse, and why.

## Context

purlis makes 1,199 decisions about a chat (the #6970 inventory, read from `main` at
`eb9c63fe8`): 819 of them hard-coded and 247 made in more than one place. Two different things
are already called "policy" (the project file's `[sandbox] policy` and the administrator's
`/etc/purlis/policy.json`), the admin locks reach only the sandbox and dispatch, and about 15
stores hold grants, nevers and remembered answers, most of them writable by an unsandboxed chat.
The same rule behaves differently per harness (#6972).

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
  locks in CI (#6971).
- The principal is always a **Chat**, in its Workspace, Persona, Harness, Runner and Plane. The
  person is never a principal (Q311 point 2).
- An **ask** is a forbid carrying `@ask("you")`. The outcome is ask when every deciding forbid
  asks, deny when any doesn't, and allow otherwise, so deny > ask > allow follows from Cedar's own
  forbid-beats-permit (Q311 point 6). An ask with no permit beneath it is a deny (Q318 point 2).
- Every rule carries `@id`, and every outcome names the rule that decided it. The shipped default
  allows by default and ends in the catch-all permit `everything-else`, so even a default deny is
  named (Q311 point 1, Q316 point 1, Q318 point 1).
- Each `[policy]` family is a Cedar action group of the same name, and its mode (`off`, `log`,
  `enforce`) is applied after Cedar decides (Q311 point 8). There are 25: ADR 0125's 24 and
  `audit`, which holds limits only and takes no `off` or `log` (Q313 point 4).
- A family's mode acts on that family's forbids and asks only. Its permits always count, so
  turning a family off never denies; a permit stops counting only when `[policy.rules]` names it,
  as deny-by-default's `"everything-else" = "off"` does (Q319 C2, amending Q311 point 8, and Q318
  point 1).
- An `@ask` forbid on a read, write, connect or socket action takes the `asks` family's mode; the
  action stays in its own family for everything else (Q319 C4).

**Rejected.** OPA/Rego through regorus: pre-1.0, no allow/deny conflict model, no equivalence or
reachability analysis, an unfinished explanation API. A TOML evaluator of purlis's own: the
fastest (0.5 µs against Cedar's 94 µs p50), but every guarantee above would be custom code, against
CLAUDE.md's second priority (#6971).

**Hard to reverse because** rules, the schema, the shipped default, the tests' oracle and every
refusal's wording name Cedar's entity types, actions and `@id`s. Leaving Cedar would mean
rewriting every project's `policy.cedar`.

### 2. The policy in force

- The project's rules live in one committed `policy.cedar` beside the project file at the plane
  root, read as committed at the plane root's HEAD, never from a branch folder (Q309 point 1).
- The text the person last accepted on this machine is kept **sealed**. Every decision is the
  stricter (deny > ask > allow) of the committed policy and the accepted policy, so a tightening
  applies at once and a loosening waits for the person's **loosening press**. The shipped default
  stands in until the first acceptance (Q309 point 2).
- Only the session host loads, verifies and evaluates it. Hooks, the proxy and dispatch ask the
  host, and an action is refused if the host can't be reached (Q309 point 3). This replaces ADR
  0125 §3's `$PURLIS_POLICY` hand-off.
- A team changes the policy through git: anyone, a chat included, may propose a change in a
  branch, and each machine's window shows the rule-by-rule diff, the commit and its author, and
  accepts it with one press. purlis enforces no commit review (Q309 points 5 and 6, Q313 point 2).

**Hard to reverse because** the seal, the press and "stricter of two" are what make Q307 Q4 hold:
**a chat can never change the policy that governs it**. It holds fully on macOS, and on Linux and
Windows with the seal opt-in on (§7, Q319 C6). Any later design must keep that.

### 3. Levels and locks

- Five **levels**, top to bottom: **Organisation** (an admin-owned folder per OS, delivered by
  MDM and never fetched), **Project**, **Workspace**, **Persona** and **You**. A chat is not a
  level (Q310 points 1, 2 and 6).
- The Organisation's folder holds two files: `policy.cedar` for its rules, and `policy.toml` for
  `[policy]`, `[policy.locks]` and `[policy.limits]` in the same syntax a project uses, typed and
  validated. The Cedar files stay pure rules, for the validator and SymCC (Q319 C5).
- Workspace and Persona defaults are written in the project's committed file, as
  `[policy.workspace.<w>]` and `[policy.persona.<p>]`; workspaces have no files of their own and
  persona.md holds no permissions (Q319 C3, Q309 point 1, Q315 point 3).
- For defaults (the template and family modes) the nearest level wins, and the stricter wins
  when a workspace and a persona both set one. For rules, deny wins at every level (Q310 point 3).
- Organisation loosens without a press; Project, Workspace and Persona loosen only after it; You
  only tightens. A level writes rules only about its own chats (Q310 point 4).
- Organisation and Project may **lock** a family, an action, a grant scope or timed grants. A
  lock freezes the default below it and drops a lower permit on the locked key, with a Notice.
  Tightening is never locked (Q310 point 5, Q314 point 10). Locks replace "admin locks".
- A broken file is never skipped: a broken Project, Workspace or Persona file falls back to the
  copy the person last accepted, else `strict`; a broken Organisation file reads as `strict` plus
  every lock; the start is not refused (Q310 point 7, Q309 point 7).

### 4. Grants are the person's sealed answers

The policy decides allow, ask or deny. A **grant** is the person's answer to an ask, kept sealed
on this machine; it is used only where the policy says ask, and never lifts a deny, a lock, a
never, the kill switch or a grader bundle (Q308b, Q312 point 2, Q314 point 11).

- Precedence: lock > the person's never > policy deny > policy allow > sealed grant > policy ask
  (Q308b; its "same persona" step is a rule of the shipped default).
- Grant scopes: Once, timed (at most 1 h, with a reason), This chat, This workspace and
  Everywhere, capped per rule by `@keep` and `@timed` (Q312, Q314 points 6 to 9).
- Every ask offers **Never**, a sealed rule of the person's own that holds in every mode, `open`
  included (Q314 point 12, Q308b).
- A chat is asked only when it is attended and a window can be reached; otherwise it is refused
  and the ask goes to needs-you (Q314 point 2).
- The four answer files become **one sealed answer store**, on this machine (Q315 point 5).

**Hard to reverse because** a grant's meaning (one rule `@id`, one action, one resource) is
written into every sealed answer the person gives.

### 5. What stays outside the policy

Only three things (Q308b, Q314 point 16):

1. **A chat can't change its own policy**, which includes that a chat never answers an ask or
   makes a grant; it holds fully on macOS, or with the seal opt-in on (§7);
2. the **kill switch**, the person's emergency stop;
3. the **grader bundles**: a skill can't grade itself.

Everything else is a rule, a limit, a sealed answer or data, and a project may loosen anything
(Q307 Q4). From Q307 Q6 on, no fix adds a hard-coded limit: it names its future policy key and
default.

### 6. A clean cut

Every setting becomes a rule (`policy.cedar`), a limit (`[policy.limits]`), a sealed answer or
data, and stays a plain setting only if it decides nothing a chat may do (Q315 point 1).
`[sandbox]` is gone, `persona.md` holds no permissions, and there is no converter and no release
that reads both. A file that still holds a moved key reads as broken, with a Notice naming the
key, its line and its new home. The old answer files are not carried over (Q315 points 2, 3, 5,
11 and 12). Only the Organisation's `/etc/purlis/policy.json` is still read, translated in memory
(Q310 point 6, Q315 point 6).

**Hard to reverse because** once released, there is no reader for the old keys.

### 7. Known gaps are declared, and the chat still starts

A key a harness can't enforce is a **known gap**: it is declared in Settings and doctor, and the
chat still starts. A project that wants a refusal writes a rule on `start` that refuses it while a
gap affects a family it enforces; the shipped default has none (Q319 C1, amending Q307 Q5's "the
start is refused").

The seal is one such gap. On Linux and Windows no standard store binds a key to an unpackaged
app (#6986), so by default the seal stops hand edits and sandboxed chats only, and the gap is
declared. Two opt-ins close it: "Confirm with Windows Hello each time purlis starts" on Windows,
built first, and "Ask for a password each time purlis starts" on Linux (Q319 C6).

**Hard to reverse because** the gap cells, Settings and doctor all assert "declared, and started":
refusing by default later would stop chats that start today, on every project that hasn't written
the rule.

## What it amends

- **ADR 0125:** §1's "`[policy]` has no `sandbox` key" (`[policy] sandbox = off|floor|full`
  replaces `[sandbox] mode`, Q315 point 2, Q318 point 9); §3's `$PURLIS_POLICY` (the host
  evaluates, Q309 point 3); its enforcement log, folded into the event log with no store of its
  own (Q313 point 3). Each lands as an amendment file under `docs/adr/0125/`.
- **ADR 0124:** §3's "Project recommends" row and item 3, the wording of §4.1 and §4.6, and §9's
  "timed exception where it exists" (Q312 point 3), as an amendment file under `docs/adr/0124/`.
- **Q307 Q5's** "the start is refused" (Q319 C1) and **Q311 point 8's** family `off` "skips the
  family's rules" (Q319 C2), as rulings rather than records; ADR 0126 states the amended reading.
- **CONTEXT.md:** "Policy" widens to the whole statement; "admin lock" becomes **Lock**; **Level**
  gains Organisation and You (the M75 spec lists every entry).

## Consequences

- A refusal, an ask and an allow each name a rule `@id`, its level and its owner, and
  `purlis policy why` answers for one action without running it.
- Enforcement is proved equal to the policy: equality cells take their expected outcome from an
  independent Cedar oracle, a looser mismatch is an escape (a public p0), and each of the 24
  known harness gaps is a gap cell asserting its declaration (Q317, Q319 C1).
- On Linux and Windows, unless the person turns on the seal opt-in, an unsandboxed program
  running as the same user can read the seal key. That is a declared gap, shown in Settings and
  doctor (Q309 point 4, Q319 C6, #6986).
- The sandbox's paths are compiled from the policy with Cedar's typed partial evaluation, which
  is experimental, behind one function and a differential test (#6988). A rule the sandbox can't
  express (one that reads `attended` or `mode`, or uses `like`, on a sandbox action) is refused
  at load.
- A decision costs about 94 µs p50, about 0.2 ms with the two evaluations (Q311).
- Every existing settings file with a moved key reads as broken until a person rewrites it.
