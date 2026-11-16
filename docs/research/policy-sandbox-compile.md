# Which paths may this chat read? Compiling the sandbox from the policy

Research note, 2026-11-16, for #6988 under the map #6969 ("One project policy governs everything
a chat can do"). It answers #6971's open risk 3:

- the OS sandboxes (Seatbelt, the Linux wrap) and Claude Code's sandbox need the **whole set** of
  paths a chat may read and write, and the hosts it may reach, not one yes or no per action;
- how does purlis get that set from the Cedar policy that ruling Q311 fixes?

The rules this note assumes are Q311's (`work/who-judges/decisions.md` on the plane):

- the principal is always a Chat, in its Workspace, Persona, Harness, Runner and Plane;
- a Path's parents are its folders and its owning workspace, repo and branch folder (hierarchy,
  not `like`);
- context is `sandbox`, `attended`, `machine`, `mode` and `depth`;
- an ask is a `forbid` with `@ask`.

## How to read this note

- **[cited]** marks a claim read this session in a primary source:
  - `cedar-policy` 4.14.1's own source (the rustdoc docs.rs renders) and its `Cargo.toml`, from
    the local Cargo registry at the exact version the engine research pins;
  - Cedar's CHANGELOG on `main`, RFC 95 and the RFC process README, read from GitHub;
  - the Lean model in `cedar-policy/cedar-spec` (`main`);
  - `cedar-policy-symcc`'s `lib.rs` at tag `v4.14.1`.
- **[measured]** marks the scratch crate below.
- **[repo]** marks purlis's code, by path and line, at `eb9c63fe8`.
- **[inference]** marks my reasoning, which is not established until it is built.

**Not read:** docs.rs and docs.cedarpolicy.com. Fetching docs.rs was refused in this chat, so I
read the crate's own doc comments, which are the same text. docs.cedarpolicy.com was not tried
after that. Nothing below rests on a page only those sites carry.

## The answer

1. **(a) Typed partial evaluation (TPE) works for paths, used one way:**
   - Call `PolicySet::tpe` once per chat start for each sandbox action (`read`, `write`).
   - Pass the chat as principal and the start-time context as known, and leave the resource as
     an unknown `Path`.
   - Then read the residual policies. They come back as plain `resource in <entity>` tests.
   - Do **not** use `query_resource`, which lists only entities it is handed.
   - It is experimental. Its API changed in a breaking way in 4.8, 4.12 and 4.13. Its semantics
     are proved sound in Lean.
2. **(b) The subset.** Rules on a sandbox action are compiled only if every residual is `true`,
   `false`, `resource in E` or `resource == E`, where E is a Path, Workspace, Repo or Branch.
   Then the sandbox is the union of the permits minus the union of the forbids, which is what
   Cedar's "forbid overrides permit" means. Seatbelt and Claude Code already apply grants and
   denials the same way.
3. **(c) The proof** is in layers:
   - the subset is checked at load, by construction;
   - a differential test in CI compares Cedar's own decision with the compiled set over
     generated hierarchies;
   - Lean's TPE soundness theorem covers the evaluator;
   - M74.1's cells, real operations in the real sandboxes, cover what the OS does with the
     profile.

   SymCC proves things about the policy (locks, "no chat ever reads a vault"), not about the
   compiled profile.
4. **(d) Context.**
   - `sandbox`, `machine` and `depth` are fixed when a chat starts, so TPE folds them away.
   - `attended` and `mode` can change while the chat runs [inference]. An OS profile can't
     follow them, and a hook never sees a file a command opens.
   - So a rule on `read`/`write`/`connect` that names them is **refused at load**, under Q310
     point 7. Falling back to the hook would leave commands unguarded.
   - An `@ask` on a Path compiles to a deny plus today's sandbox Block with its Allow. Hosts are
     asked live by the proxy, as today.
5. **(e) Recommendation.**
   - Compile **paths** with TPE, pinned and wrapped in one purlis function. Behind it sits the
     differential test, and the stable-API scope compiler as the fallback.
   - **Do not compile hosts at all.** The egress proxy already decides each connection, so it
     asks the host's Cedar per connect (Q309 point 3).
   - Refuse the older Claude Code that still needs a host list.
   - Keep reads at "everything but the forbids" until each harness can take a read allow-list.

## What purlis compiles today

**The neutral form already has the shape a policy compiles to** [repo]:

- `Compiled` (`crates/purlis-core/src/sandbox.rs:3952`) carries the folders a chat may write
  (`writable`), the paths it is denied (`denied`) and the hosts with their layer (`reach`).
- It also carries the sockets and ports a person granted (`Widened`, `sandbox.rs:3983`).
- Each `Denial` (`sandbox.rs:1520`) names a path, an `Access` and a `Reach`:
  - `Access` is `ReadWrite` or `Write`;
  - `Reach` is `AndBelow` (the subtree) or `Itself` (the entry alone, `sandbox.rs:1030`).
- It also has an `any_case` flag for case-folding disks.

**Seatbelt** (`sandbox/seatbelt.rs`):

- It takes the last rule that matches, so "every denial comes after every grant" (`:8-9`).
- Writes are an allow-list of `subpath`s (`:533-549`) with denials after them.
- Reads are `(allow file-read*)` (`:1004`), narrowed only by `file-read*` denials (`:213`).
- Network is the proxy's two loopback ports and named unix sockets (`:500-505`).

**Linux** [repo]:

- The supervisor reads **the same Seatbelt profile text**, last match wins, and judges every
  file operation and connect the chat makes (`crates/linux-sandbox/src/profile.rs:1-18`).
- The chat's own Landlock layer only starts programs (`linux/layers.rs`, `chat`). So one
  compiled artifact serves both systems.

**Claude Code** (`sandbox/claude.rs`):

- Bash commands are held by `sandbox.filesystem.denyRead` / `denyWrite` / `allowWrite`. Its
  Read and Edit tools are held by `permissions.deny` (`:9-11`, `:472-476`).
- `denyWrite` wins over `allowWrite` (`:333`, `:76-81`).
- purlis writes no read allow-list for Claude Code, only `denyRead`. I did not check whether
  Claude Code's schema has a read allow-list.

**Hosts** [repo]:

- The proxy decides each connection by host and port (`sandbox/egress.rs:1-40`), through
  `Reach::decide` (`sandbox/reach.rs:1-25`).
- Claude Code from 2.1.285 hands every command connection to that proxy. The `allowedDomains`
  list is still written, because Claude Code restricts the network only while it is set
  (`claude.rs:56-72`, `:631-638`).

So the OS needs **sets of path subtrees**, with forbid after permit. For hosts it needs only
"the proxy's ports" [inference from the above].

## (a) Typed partial evaluation in cedar-policy 4.14.1

### What the API offers [cited]

- `tpe` is a Cargo feature, listed under **"Experimental features … unstable and subject to
  breaking changes in any release, including patch releases"** (`cedar-policy-4.14.1/src/lib.rs`,
  crate docs; `Cargo.toml` `[features]`). The older `partial-eval` says "You should prefer `tpe`".
- `PolicySet::tpe(&PartialRequest, &PartialEntities, &Schema) -> TpeResponse`
  (`src/api/tpe.rs:738`):
  - `TpeResponse::decision()` is `Some(Allow|Deny)` or `None`;
  - `residual_policies()` returns the non-trivial residuals;
  - `reason()` and the `true_*`/`residual_*` iterators name policies (`:420-640`).
- In a `PartialRequest`, the "principal/resource types and action must be known and its context
  must either be fully known or unknown" (`:70-72`).
- `PolicySet::query_resource` (`:777`) runs `tpe`, then **filters the entities you pass it** and
  re-authorises each one (`:786-820`). It can list only Paths that exist as entities.
- RFC 95 describes the design ([RFC 95](https://github.com/cedar-policy/rfcs/blob/main/text/0095-type-aware-partial-evaluation.md)):
  - context is all known or all unknown;
  - absent entities are "present entities with unknown contents";
  - its Timeline says "Accepted: TBD, Stabilized: TBD".
- With the context unknown, the result is what "*may* be allowed in *some* context" (RFC 95,
  "Permissions queries with TPE").
- **Soundness is proved in Lean.** `theorem reauthorize_is_sound` in
  `cedar-spec/cedar-lean/Cedar/Thm/TPE.lean` says re-authorising a partial response with the
  concrete request "is equivalent to directly authorizing with the concrete request and
  entities".
  - There are also proofs that residuals are well-typed and error-free (`Thm/TPE/WellTyped`,
    `ErrorFree`).
  - The RFC process says an experimental feature "may not yet be reflected in the formal model
    or Cedar DRT". TPE is in the model; whether the Rust is differentially tested against it is
    not verified.

### How stable it is [cited, CHANGELOG on `main`]

| Version | Date | What changed in `tpe` |
|---|---|---|
| 4.6.0 | 2025-10-06 | Introduced (RFC 95), with batched evaluation |
| 4.7.0 | 2025-10-13 | `query_action`, `PartialEntities::empty` |
| 4.8.0 | 2025-11-17 | **Breaking:** `PartialEntities::from_concrete` now needs a `Schema`. `residual_policies` added |
| 4.9.0 | 2026-02-09 | Simplifies `<residual> && false` and `\|\| true` |
| 4.11.0 | 2026-05-18 | Template-linked policies. Residuals convert to the new `pst` syntax tree |
| 4.12.0 | 2026-07-28 | **Breaking in meaning:** `residual_policies` returns only non-trivial residuals, and `nontrivial_residual_policies` is deprecated. `reason` and the iterators are added |
| 4.13.0 | 2026-09-15 | **Breaking:** a `BatchedEvalError` variant removed. Fixes `policy_set` returning the *original* policies, not the residuals |
| 4.14.0 | 2026-10-17 | Reduces `has`, `in` and `==` on unknowns when the schema decides them |

That is about one breaking change every two or three minors, and one fix (4.13) where an
accessor returned the wrong thing. With `cedar-policy` pinned exactly (Q311), this costs work
only at a bump. Every bump must re-run the differential test below [inference].

### What it does on Q311-shaped rules [measured]

**The scratch crate:**

- `cedar-policy = "=4.14.1"` with `features = ["tpe"]`, in my scratch folder, outside the repo.
  Its `target/` was deleted afterwards.
- **Schema:**
  - `Chat in [Workspace, Plane]`;
  - `Path in [Path, Workspace] { name: String }`;
  - actions `read` and `write` in an action group `files`;
  - context `{sandbox, attended, machine, mode, depth}`.
- Both rule sets passed **strict** validation.
- The rules:

```cedar
@id("own-workspace") permit(principal in Workspace::"a", action in [Action::"read", Action::"write"], resource in Workspace::"a");
@id("plane-read")    permit(principal, action == Action::"read", resource in Path::"/p");
@id("home-read")     permit(principal, action == Action::"read", resource in Path::"/home");
@id("no-vaults")     forbid(principal, action, resource in Path::"/p/.purlis/vaults");
@id("no-ssh")        forbid(principal, action, resource in Path::"/home/u/.ssh");
@id("no-other-workspace") forbid(principal in Workspace::"a", action == Action::"read", resource in Workspace::"b");
// the ones an OS sandbox cannot hold as written:
@id("cross-ws-ask") @ask("you") forbid(principal in Workspace::"a", action == Action::"read", resource in Workspace::"c") unless { context.attended };
@id("plan-mode")     forbid(principal, action == Action::"write", resource) when { context.mode == "plan" };
@id("deep-dispatch") forbid(principal, action == Action::"write", resource in Path::"/p") when { context.depth > 2 };
@id("dotenv")        forbid(principal, action == Action::"read", resource) when { resource.name like ".env*" };
```

**What TPE returned** for Chat `c1` in Workspace `a`, with the resource an unknown `Path`:

- **Context known** (`attended: false, mode: "default", depth: 1`), `read`, all rules:
  - decision `None`;
  - every principal scope and action scope folded away, and every context test folded away;
  - the residuals are exactly:

    ```
    permit … when { resource in Path::"/p" }            // plane-read
    permit … when { resource in Path::"/home" }         // home-read
    permit … when { resource in Workspace::"a" }        // own-workspace
    forbid … when { resource in Path::"/p/.purlis/vaults" }
    forbid … when { resource in Workspace::"b" }
    forbid … when { resource in Path::"/home/u/.ssh" }
    @ask("you") forbid … when { resource in Workspace::"c" }   // unless attended, folded
    forbid … when { resource.name like ".env*" }               // not a scope: see (b)
    ```

  - `plan-mode` and `deep-dispatch` evaluated to false and dropped out. Annotations (`@id`,
    `@ask`) are kept on residuals.
- **`write` in plan mode at depth 3:** decision **`Some(Deny)`**. The chat may write nothing,
  and TPE says so outright.
- **Context unknown:** `cross-ws-ask` stays as `(resource in Workspace::"c") && (!context.attended)`,
  the "some context" answer RFC 95 describes.
- **No Path entities at all** (only Plane, Workspace `a` and Chat `c1`): the same residuals. TPE
  needs the principal's ancestry, not the file tree.
- **Speed:** 20–34 µs per call with 25 Paths, and **29–30 µs p50 with 2,160 Paths**. That is
  independent of the tree, since residuals never touch Path entities. The machine was the same
  oversubscribed 12-core arm64 as the engine research.
- **`query_resource` over 2,160 Paths:** about **23–24 ms p50**, returning 2,021 of them. It can
  only list Paths that exist as entities. A file made after the chat starts, or any file purlis
  didn't enumerate, is not in its answer. It is the wrong tool for an OS profile, which speaks
  in subtrees.

**So TPE gives the set, as residuals.** "The set of Paths this chat may read" is the union of
the permits' `resource in X` minus the union of the forbids' `resource in Y`, and purlis turns
each X into its folder [inference, checked in (c)].

## (b) Compiling from rule scopes: the subset

### The subset

Rules on a sandbox action (`read`, `write` on Path; `connect` only if hosts were compiled, which
(e) advises against) compile when **every residual condition, after TPE with the start context,**
is one of:

| Residual | Compiles to |
|---|---|
| `true` (an unconditional `resource` scope) | the whole tree, `/` |
| `false` | nothing: the rule is dropped |
| `resource in Path::"p"` | `p` and below (`Reach::AndBelow`, Seatbelt `subpath`, Claude Code's path) |
| `resource == Path::"p"` | `p` alone (`literal`). For a forbid this is not `Reach::Itself`, which is "never created, renamed, moved or removed" (`sandbox.rs:1030`) |
| `resource in Workspace/Repo/Branch::"x"` | the folder(s) the path resolver gives that entity |
| a disjunction (`\|\|`) of the above | the union |

Anything else **is refused at load** for that action, with the rule's `@id` named. That covers:

- `like` on a Path's name or attribute;
- `has`;
- `&&` of two scopes, which would be an intersection;
- any test on a non-start context value (see (d)).

The refusal goes through Q310 point 7 (fall back to the accepted copy, else `strict`, with a
Notice). The other route, Q307 Q5's "known gap, start refused", is coarser, because the rule
could still serve tool calls [inference].

### Why the result is Cedar's answer

Cedar's decision is "allow iff some permit is satisfied and no forbid is"
([authorization](https://docs.cedarpolicy.com/auth/authorization.html), as cited in the engine
research).

- With every residual a subtree test, that is `(∪ permits) \ (∪ forbids)` exactly.
- Seatbelt expresses it by putting each denial after each grant (`seatbelt.rs:8-9`).
- Claude Code expresses it by `denyWrite` beating `allowWrite`.
- No ordering of rules matters, and no narrower permit can punch through a forbid. That is
  also what Q311 point 6 says of asks [inference].

### Two things the subset needs from purlis

- **One resolver.** `resource in Workspace::"a"` is only a path once purlis says which folders
  Workspace `a` owns. The compile and the hook's request builder must use **the same** resolver:
  - canonical real path;
  - macOS firmlinks (`/Users` vs `/System/Volumes/Data/Users`, which `seatbelt.rs` already
    spells both ways);
  - case folding.

  The invariant to test: a Path's Cedar ancestors are exactly its folder prefixes plus the
  workspace, repo and branch folder whose root is one of those prefixes. Otherwise
  `resource in Path::"p"` (Cedar) and `subpath p` (OS) disagree [inference].
- **Reads stay a deny-list until each backend can take an allow-list.**
  - Today Seatbelt reads `(allow file-read*)`, and Claude Code gets only `denyRead`.
  - A policy whose read permits don't cover `/` asks Seatbelt to allow-list the harness's own
    system reads (dyld, `/System`, `/usr`), which purlis has never written.
  - For Claude Code it can only be written as a `denyRead` of the complement, which is unbounded
    and misses files made later.
  - So the shipped default should `permit read` on `Path::"/"` and narrow by forbids. A
    read-permit narrower than `/` is, for now, a declared gap on Claude Code (start refused, Q5),
    until its schema is shown to take a read allow-list [inference; Claude Code's schema not
    re-read here].

### The same compile without TPE (the fallback) [measured]

- With only the **stable** API, `Policy::principal_constraint`, `action_constraint` and
  `resource_constraint` give the scope.
- The JSON form's `conditions` array says whether a rule has a `when`/`unless`.
- The scratch crate's `compile_scopes` does this:
  - it matches the principal scope against the chat's ancestors and the action against the
    action and its group;
  - it maps `In(Path)` to a subtree, `In(Workspace)` to its folder, and `Eq` to a literal;
  - it **refuses** any rule with a condition.

On the six scope-only rules it gave:

- `read`: allow `{/p, /home, /p/workspaces/a}`, deny `{/p/.purlis/vaults, /home/u/.ssh, /p/workspaces/b}`;
- `write`: allow `{/p/workspaces/a}`, deny `{/p/.purlis/vaults, /home/u/.ssh}`.

**0 mismatches** against `Authorizer::is_authorized` on all 25 Paths, for both actions.

With the four conditional rules added, it refused `cross-ws-ask` and `dotenv` on `read`, and
`plan-mode` and `deep-dispatch` on `write`. Comparing anyway (ignoring the refused rules) gave
**6 mismatches on read**, every one looser than Cedar (`/p/workspaces/c/**`, `/p/workspaces/a/.env`).
That is why a refused rule must stop the compile, never be skipped.

The cost of this fallback is that purlis re-implements principal, action and context matching,
which is a second evaluator. Q311 wanted Cedar to be the only one. TPE avoids that while it holds
[inference].

## (c) Proving that the compiled sandbox equals the policy

Four layers, each covering what the one before can't:

1. **By construction, at load:**
   - the TPE residual of every rule on a sandbox action is in the subset of (b), or the file is
     refused;
   - inside the subset, set algebra is Cedar's decision (above);
   - this is a check on syntax, cheap and in the host.
2. **Differential test, in CI, no solver** [measured as a prototype; the generator is
   inference]:
   - generate hierarchies (folders, workspaces, repos, branch folders) and rules in the subset;
   - for each chat and action, compile, then compare `Authorizer::is_authorized` with
     set membership:
     - on every Path entity;
     - on a fresh child below each permit and forbid root, standing in for files made later;
     - on the roots' siblings.
   - The scratch crate's check (D) is the seed: 25 Paths, 0 mismatches.
   - This is the contract any `cedar-policy` bump must pass, and it is what catches a TPE
     regression like 4.13's `policy_set` fix.
3. **TPE's own soundness:** the Lean theorem `reauthorize_is_sound` (above) [cited]. Layer 2
   checks the Rust against Cedar's concrete authorizer, so purlis does not lean on DRT coverage
   of `tpe`, which is unverified.
4. **The OS side:** compiled set → Seatbelt text → what the kernel does.
   - This is where symlinks, firmlinks, case folding and mounts live. No Cedar tool sees it.
   - It is M74.1's matrix: real reads and writes in real sandboxes, per harness and OS, under
     `strict`.
   - Since Linux judges by the same profile text (`profile.rs`), one compile is tested on two
     systems. Claude Code is the third backend and needs its own cells.

**SymCC** (`cedar-policy-symcc`, "every well-formed input in the `symenv`": `check_implies`,
`check_equivalent`, `lib.rs` at `v4.14.1`) [cited]:

- Its place is **policy-level** invariants that bound the compile: "no Chat ever reads
  `Path::"…/vaults"`" (`check_never…`), and locks (`check_implies`, Q310 point 5).
- It can't prove the compile, for three reasons [inference]:
  - its inputs are all well-formed hierarchies, not this chat's;
  - it doesn't model the OS;
  - restating the compiled set as Cedar and checking it against the residuals would only
    restate layer 1.
- It needs `cvc5`, so it belongs in CI (engine research, risk 7).

**Sampling actions "both ways" at run time** (asking Cedar per hook call and comparing with the
profile) is not a proof:

- it sees only the tool calls a hook sees, never a command's own file operations;
- a mismatch found there is found after the fact.

It is useful as an M74.1 cell or a `log`-mode counter, not as the guarantee [inference].

## (d) `when` clauses the OS sandbox can't express

Split the context by when it is known [inference, from Q311 point 5 and the code above]:

| Context | Known when | In a sandbox action's rule |
|---|---|---|
| `sandbox` (off/floor/full) | chat start: it sets the profile | **Folded** by TPE |
| `machine` | chat start | **Folded** |
| `depth` | chat start: a dispatched chat's depth is fixed | **Folded** (as `deep-dispatch` above) |
| `attended` | may change while the chat runs | **Refused at load** |
| `mode` (harness permission mode) | may change while the chat runs (the person can switch it) | **Refused at load** |

**Why refuse rather than "fall back to the hook":**

- A Seatbelt profile is fixed for the process. A second profile can't be applied inside it
  (`seatbelt.rs:11-14`). Claude Code's settings are fixed at its start.
- The hook sees the harness's tool calls (Read, Edit), not the files a Bash command opens.
- So a rule like `forbid write when mode == "plan"` could hold only for tool calls. The OS layer
  would have to pick one of two wrong answers:
  - the loosest compile, where commands write in plan mode, which breaks the rule;
  - the strictest, where commands never write, which breaks every other mode.
- A rule enforced only on some paths is the partial enforcement Q307 Q5 says to declare up front
  rather than hide. Refusing at load is that declaration, with the rule named.

**The same verbs off the OS path keep full context.** Rules for tool calls in the `mode`,
`tools` and `guards.*` families are decided per action in the host, with the whole context, and
need none of this.

**Asks and attendance:**

- `attended` rarely needs to be in a rule, because Q308b decides it at the ask. An attended chat
  gets the Notice; an unattended one gets a refusal plus a needs-you item.
- An `@ask` forbid on a Path compiles to a **deny** in the profile (an OS can't pause for a
  person on macOS). It surfaces through today's sandbox Block, whose Allow becomes a sealed grant
  (Q308b) [repo: `sandboxblock*.rs`; the grant path is Q308b's].
- On Linux the supervisor already holds each operation (seccomp notify), so it *could* ask live.
  That is a later improvement, not needed for equality [inference].
- On a Host, the proxy already holds the connection and asks (`reach.rs:22-25`) [repo].

**`like` and attributes (`dotenv` above)** are outside the subset, so they are refused too.
That bites:

- purlis's later-code names (`**/.git/config`, `.envrc`, `.mcp.json`, held at any depth;
  `sandbox.rs:999-1004`, `claude.rs:31-39`) are such patterns;
- under Q307 Q4 they must become policy.

The way in is **one form at a time**: a rule shape like a name test on `resource.name`, added
to the subset only with a compiler for all three backends (Seatbelt `regex`, Linux the same text,
Claude Code `**/<name>`) and its own cases in layer 2 [inference]. That is spec work, not this
note's.

## (e) Recommendation

1. **Paths: compile with TPE, behind one function.**
   - Something like `sandbox_paths(chat, action, start_context) -> Compiled`, the only caller of
     `tpe`, in `purlis-core`.
   - It is called once per chat start for `read` and `write`. It is about 30 µs and needs no
     Path entities.
   - It accepts only the subset of (b), and feeds today's `Compiled`/`Denial` form. So Seatbelt,
     the Linux supervisor and Claude Code's settings keep their compilers unchanged.
   - Pin `cedar-policy` exactly (Q311). Run the differential test of (c) on every bump.
   - Keep the stable-API scope compiler as the fallback if a bump breaks `tpe`, with the same
     subset and the same test. Its cost is purlis owning a small second evaluator.
2. **Hosts: don't compile.**
   - The egress proxy already decides each connection by host and port.
   - Under Q309 point 3 it asks the session host, which evaluates Cedar with the full context
     (about 94 µs, engine research).
   - The OS profile only ever needs the proxy's two ports.
   - Refuse a Claude Code older than 2.1.285, which needs a host list of its own
     (`sandbox.rs:4643-4645`) and is already told to update.
3. **Ports and sockets** (`Widened`) follow the paths' recipe if they become Cedar resources:
   TPE with `resource == Port::"…"` residuals [inference].
4. **Context:**
   - fold `sandbox`, `machine` and `depth`;
   - **refuse** `attended`, `mode` and any non-scope test on a sandbox action at load, naming
     the rule;
   - compile `@ask` on a Path to deny plus the Block and its Allow.
5. **Reads:**
   - the shipped default permits `read` on `/` and narrows with forbids;
   - a narrower read permit is a declared gap on Claude Code until its settings take an
     allow-list.
6. **Proof:**
   - the subset check at load;
   - the differential test in CI;
   - the Lean TPE theorem as the evaluator's guarantee;
   - M74.1's cells for the OS;
   - SymCC in CI for policy invariants and locks.

## Open points for the spec

- **Is `attended` really mutable within one chat, and `mode`?** If purlis fixes either at
  start, it moves to the "folded" row.
- **The resolver's exact rules,** shared by the compile and the hook: symlinks, firmlinks, case,
  and a workspace with more than one folder. Also branch folders under `.worktrees`, whose Path
  has two owners (workspace and branch folder).
- **Later-code names as policy:** the first `like`-style form for the subset (see (d)).
- **Whether Claude Code's sandbox schema takes a read allow-list.** Not re-read here.
- **Whether `tpe` is in Cedar's differential testing (DRT)** against the Lean model, or only
  proved there.
- **Not read this session:** docs.rs and docs.cedarpolicy.com (see "How to read this note").
