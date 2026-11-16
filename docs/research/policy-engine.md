# Which engine evaluates the policy: Cedar, OPA/Rego (regorus), or TOML with a purlis evaluator

Research note, 2026-11-16, for #6971 under the map #6969 ("One project policy governs everything
a chat can do"). It answers which engine should decide a policy key, given ruling Q307
(`work/who-judges/decisions.md` on the plane). Q307's main points are:

- every decision about a chat is a policy key;
- the effect is **allow / ask whom / deny**;
- a rule is **who** does **what** to **what**, **when**;
- every outcome names the rule that decided it;
- the outcome is the same in every harness.

## How to read this note

- **[cited]** marks a claim read this session in the primary source linked. The Cedar docs, the
  `cedar-policy` and `cedar-policy-symcc` docs on docs.rs, the two Cedar papers on arXiv, the
  OPA docs, regorus's README and source on GitHub, and the crates.io API were all read on
  2026-11-16.
- **[measured]** marks the benchmark below.
- **[repo]** marks something this repository records.
- **[inference]** marks my reasoning, which is not established until someone builds or
  measures it.

The **TOML** option means a purlis-defined declarative format with an evaluator written in
purlis. No outside source describes it, so its column says what purlis would have to build.

## The answer

**Use Cedar (`cedar-policy`, Apache-2.0), embedded in the host and parsed once.**

- It is the only candidate whose own semantics already give two of Q307's demands:
  - deterministic, order-independent evaluation, proved in Lean;
  - "deny overrides" across layers.
- It names the policies that decided an outcome.
- It validates against a schema, with a soundness proof.
- It has a decidable analyser for "can any chat ever do X?" and for whether two policies are
  equivalent.
- One decision over about 200 Q307-shaped rules costs about **94 µs p50 and 130–250 µs p99**,
  measured on a machine at load average 27–35 on 12 cores. That is far inside a few
  milliseconds, and the spec's whole hook call budget is ≤ 50 ms (`docs/spec.md`, "Hook call")
  [repo].

Purlis still has to own three conventions, because Cedar knows only `permit` and `forbid`:

1. **Ask whom.** Write it as an `@ask("<whom>")` annotation on a `forbid`. If every forbid that
   decided an outcome carries `@ask`, the outcome is *ask* that party. If any decided without
   it, the outcome is *deny*. That makes **deny > ask > allow** (the most restrictive wins), the
   order Cedar already gives forbid over permit. It also means an organisation's ask is a
   guardrail no lower layer can turn into an allow. See [(a)](#a-allow--ask-whom--deny).
2. **Layers and locks.** Each policy carries `@layer("organisation" | "project" | "person")`.
   Cedar's forbid already beats every permit, so a higher layer's deny or ask cannot be loosened
   below it. A lock that stops a lower layer from adding a *permit* is a load-time check purlis
   makes, and `cedar-policy-symcc`'s `check_implies` can prove it in CI. See
   [(b)](#b-layers-and-locks).
3. **The default deny names a rule.** Cedar's default deny names no policy [cited], so the
   shipped default policy ends with a named catch-all, or purlis reports it as `default`.

**Trade-offs accepted:**

- A person writes Cedar, not TOML.
- Path globs are only `like` with `*`.
- `ask` and `lock` are purlis conventions on top of Cedar. Cedar does not know them, so they
  need their own tests.

**Regorus was rejected.** It is quick and pure Rust, but:

- it is pre-1.0;
- Rego has no allow/deny model and no conflict model;
- nothing in the Rego world answers equivalence or "can anyone ever" questions;
- regorus's own explanation API is unfinished.

**TOML was rejected.** It would be the fastest (0.5 µs) and the easiest to read, but every
property above would be purlis's own code and purlis's own proof. That is what CLAUDE.md
priority 2 rules out when a mature standard exists.

## Comparison

| Criterion | Cedar (`cedar-policy` 4.14.1) | OPA/Rego via `regorus` 0.15.0 | TOML + purlis evaluator |
|---|---|---|---|
| **Deterministic** | Yes, by design and by proof: "guaranteed to terminate and always produce the same authorization decision", and "policy evaluation order doesn't matter" ([OOPSLA §2.4](https://arxiv.org/html/2403.04651)). Order independence and termination are proved in Lean ([VGD §3.2](https://arxiv.org/html/2407.01688)). It has no clock, random or network builtins, so time comes in through `context`. | Rego terminates, because it has no recursion and no loops ([OPA](https://www.openpolicyagent.org/docs/errors/rego-recursion-error/rule-name-is-recursive)). OPA marks `time.now_ns`, `rand.intn`, `uuid.rfc4122`, `http.send` and others nondeterministic ([capabilities.json](https://raw.githubusercontent.com/open-policy-agent/opa/main/capabilities.json)). regorus feature-gates `time` and `uuid`, but `rand.intn` comes with `std`, and "many builtins are `baked-in`" ([builtins.md](https://raw.githubusercontent.com/microsoft/regorus/main/docs/builtins.md)). Purlis would have to forbid them by review. | It is as deterministic as purlis writes it. Nothing is proved. |
| **Embeds in plain Rust, no new runtime** | Yes. It is pure, safe Rust ([security](https://docs.cedarpolicy.com/other/security.html#security-cedar)). Its default tree has 90 crates [measured: `cargo tree`]. | Yes. It is pure Rust, `no_std`, with `forbid(unsafe_code)` ([lib.rs](https://raw.githubusercontent.com/microsoft/regorus/main/src/lib.rs)). Its default tree has 162 crates [measured], and the defaults pull in `vstd` pinned to a dated pre-release ([deps](https://crates.io/api/v1/crates/regorus/0.15.0/dependencies)). | Yes. It needs only `toml` and `serde`. |
| **Latency per decision** | Paper: median 4–11 µs and p99 under 20 µs on its benchmarks, 42–80× faster than Rego/OPA ([OOPSLA §5.2](https://arxiv.org/html/2403.04651)). **Here: p50 94 µs, p99 128–247 µs over 199 policies** [measured]. | README: RVM 1.35–2.15× faster than Cedar on adapted benchmarks, with setup excluded and policies hand-optimised ([README](https://github.com/microsoft/regorus#cedar-benchmark-comparison)). **Here, using the interpreter: p50 170 µs, p99 249–268 µs** [measured]. | **Here: p50 0.5 µs, p99 0.8 µs** [measured], for a linear scan. |
| **Static analysis** | `cedar-policy-symcc` 0.8.1 provides `check_always_allows`, `check_always_denies`, `check_implies`, `check_equivalent` and `check_disjoint`, each with counterexamples, "formally modeled and verified in Lean" ([docs.rs](https://docs.rs/cedar-policy-symcc/0.8.1/cedar_policy_symcc/index.html)). The encoding is "decidable, sound, and complete", at about 75 ms per check ([OOPSLA §1, §5.4](https://arxiv.org/html/2403.04651)). It needs an external `cvc5` binary, so it is a CI or authoring tool, not the hook path. | Nothing equivalent. "Datalog program equivalence is undecidable" ([OOPSLA §6](https://arxiv.org/html/2403.04651)). OPA has `opa check --strict`, which flags unused locals and imports, and partial evaluation ([CLI](https://www.openpolicyagent.org/docs/cli), [strict](https://www.openpolicyagent.org/docs/policy-language#strict-mode)). regorus has no checker. | Whatever purlis writes. |
| **Explains an outcome by its rule** | `Response::diagnostics().reason()` returns the IDs of the determining policies ([docs.rs](https://docs.rs/cedar-policy/4.14.1/cedar_policy/struct.Diagnostics.html#method.reason)). On Allow these are the satisfied permits; on Deny, the satisfied forbids. A **default deny is empty** ([authorization](https://docs.cedarpolicy.com/auth/authorization.html#request-authorization-algorithm)). Annotations can be read back with `PolicySet::annotation(id, key)` ([docs.rs](https://docs.rs/cedar-policy/4.14.1/cedar_policy/struct.PolicySet.html#method.annotation)). | The engine does not do it: the author returns the rule's name in the value. regorus's `explain` module is feature-gated, and its docs say "the producer API is later work" ([docs.rs](https://docs.rs/regorus/0.15.0/regorus/explain/index.html)). OPA's `--explain` is a trace, not an answer ([CLI](https://www.openpolicyagent.org/docs/cli)). | Trivial, because the evaluator picks the rule. |
| **Schema validation** | `Validator::validate(.., ValidationMode::Strict)` catches unknown types and actions, wrong principal or resource types, attribute typos and always-false policies ([validation](https://docs.cedarpolicy.com/policies/validation.html#supported-validation-checks)). Soundness is proved: an accepted policy never has a type error at evaluation ([VGD](https://arxiv.org/html/2407.01688)). | OPA type-checks against JSON Schema, but `enum`, `oneOf`, `not`, `if/then/else` and `additionalProperties` are unsupported ([OPA](https://www.openpolicyagent.org/docs/policy-language#schema)). regorus's `Schema` validates data, not policies, and only under `azure_policy` ([docs.rs](https://docs.rs/regorus/0.15.0/regorus/struct.Schema.html)). | `serde` checks shape. Cross-references, such as an unknown persona or vault, are purlis code. |
| **Licence, maturity, governance** | Apache-2.0. A 4.x release has come out about every month since 2024-09, and 4.14.1 is dated 2026-11-02. 12.0 M downloads ([crates.io](https://crates.io/api/v1/crates/cedar-policy)). Used at scale in AWS ([OOPSLA §1](https://arxiv.org/html/2403.04651)). Lean model, proofs and differential testing ([VGD](https://arxiv.org/abs/2407.01688)). Under the CNCF code of conduct, and the name is a "trademark of The Linux Foundation" ([docs](https://docs.cedarpolicy.com/)). CNCF level not verified. | MIT, Microsoft, with a CLA. **Pre-1.0**: 0.14.0, 0.14.1 and 0.15.0 all shipped within two weeks ([crates.io](https://crates.io/api/v1/crates/regorus)). "Mostly compliant" with OPA v1.2.0, with builtins missing (JWT, glob match, `rego.metadata.*`) ([README](https://github.com/microsoft/regorus)). OPA itself is CNCF-graduated, but that status is the Go project's, not regorus's ([OPA README](https://raw.githubusercontent.com/open-policy-agent/opa/main/README.md)). | No outside maturity. Purlis maintains it alone. |
| **Written by hand** | Fixed shape: `permit/forbid (principal, action, resource) when {…}` ([syntax](https://docs.cedarpolicy.com/policies/syntax-policy.html)). The paper judges it "equally or more readable" than Rego and OpenFGA ([OOPSLA](https://arxiv.org/abs/2403.04651)). Globs are `like` with `*` only ([operators](https://docs.cedarpolicy.com/policies/syntax-operators.html#operators-string-like)). | A general language with "no built-in authorization concepts such as 'allow' or 'deny'" ([OOPSLA §6](https://arxiv.org/html/2403.04651)). Conflicting complete rules are a runtime error ([OPA](https://www.openpolicyagent.org/docs/policy-language#complete-definitions)). | The easiest to read for a fixed rule shape, and nothing more expressive than purlis makes it. |
| **Allow / ask whom / deny** | Not native. Model it with an annotation; see (a). | Native: a rule may produce any value ([else](https://www.openpolicyagent.org/docs/policy-language#else-keyword)). | Native. |
| **Layers and locks** | Forbid beats permit and evaluation has no order ([authorization](https://docs.cedarpolicy.com/auth/authorization.html#request-authorization-discussion)), so higher denies are guardrails. Permit-locks are a purlis check plus `check_implies`. | No conflict model. `default` and `else` chains exist, and precedence is the author's to write ([OPA](https://www.openpolicyagent.org/docs/policy-language#default-keyword)). | Purlis code. |

## (a) Allow / ask whom / deny

Cedar's effect is only `permit` or `forbid`
([syntax](https://docs.cedarpolicy.com/policies/syntax-policy.html#term-policy-effect)), and
annotations "have no impact on policy evaluation"
([syntax](https://docs.cedarpolicy.com/policies/syntax-policy.html#term-parc-annotations))
[cited]. I found no Cedar source that documents an "ask" pattern. There are three ways to add
one:

1. **`@ask` on a forbid (recommended).** Make one `is_authorized` call.
   - If the decision is Deny and every ID in `reason()` carries `@ask`, the outcome is **ask**
     the annotated party. Several parties are a union, for purlis to define.
   - If any deciding forbid lacks `@ask`, the outcome is **deny**.
   - If the decision is Allow, the outcome is **allow**.

   This gives deny > ask > allow with Cedar's own precedence. A lower layer's permit cannot skip
   a higher layer's ask. SymCC still answers both questions:
   - "Can a chat ever do X *without asking*?" Treat ask-forbids as forbids, which is the policy
     set as written.
   - "Can it ever do X at all?" Drop the ask-forbids before the check.

   [inference]
2. **`@ask` on a permit.** Allow, unless every deciding permit carries `@ask`. This makes
   allow > ask, so a lower layer's plain permit silently removes a higher layer's ask. That
   fails Q307's locks. **The benchmark used this form.** Its cost is identical to option 1: one
   call, plus an annotation lookup per deciding policy [inference].
3. **Two evaluations.** Use one policy set for "without asking" and one for "if X approves", or
   an `ask` action evaluated in a second call. This doubles the cost and splits one rule across
   two places. There is nothing it can say that option 1 cannot.

Rego can return a decision object directly. Several matching complete rules with different
values are an error, though, so the combining rule must be written in Rego, as in the example
below. In TOML, the effect is a field.

## (b) Layers and locks

**Cedar** [cited]:

- "If any forbid policy evaluates to true, then the final result is Deny. Else, if any permit
  policy evaluates to true, then the final result is Allow. Otherwise … Deny."
- There is no priority and no ordering, and order independence is proved.
- "forbid policies effectively define permission 'guardrails' that permit policies cannot
  cross."

All three are from
[authorization](https://docs.cedarpolicy.com/auth/authorization.html#request-authorization-discussion).

What follows for purlis's layers, organisation > project > person:

- **A higher layer's deny or ask is a lock for free.** No lower permit can override it.
- **A higher layer's lock against *tightening*** is not expressible in Cedar, for example "the
  project may not forbid the person's own workspace". Neither is "a lower layer may not add a
  permit for key K". Purlis enforces these when it loads a layer:
  - it rejects a lower-layer policy whose scope touches a locked key;
  - it can prove the result with `check_implies(lower ∪ higher, higher)` in CI or at save time.

  [inference]
- **Skip on error:** "if a policy's evaluation returns error … it is skipped"
  ([authorization](https://docs.cedarpolicy.com/auth/authorization.html)). An erroring
  organisation forbid would silently not apply. Strict validation exists to prevent that
  ([security](https://docs.cedarpolicy.com/other/security.html#security-validate-against-schema)),
  so purlis must refuse a start when a policy fails validation [cited, then inference].
- **Templates** have only `?principal` and `?resource` slots, and add no layering
  ([templates](https://docs.cedarpolicy.com/policies/templates.html)) [cited].

**Rego** has no conflict model. `default` covers "undefined", and `else` gives first-match order
within one chain ([OPA](https://www.openpolicyagent.org/docs/policy-language#else-keyword))
[cited]. Layers, precedence and locks would all be Rego that purlis writes and must trust.

**TOML**: the merge and the locks are purlis code. It is the most direct fit, and it is custom.

## Q307's example in each candidate

The rule: *the chats of workspace `qa` may not read workspace `billing`'s files*, and a variant
in which they may, if the person is asked.

### Cedar

```cedar
@id("qa-may-not-read-billing")
@layer("project")
forbid (
  principal in Workspace::"qa",
  action == Action::"read",
  resource in Workspace::"billing"
);

// Variant, in place of the rule above: ask the person first. With option (a)1, a forbid
// that only asks.
@id("qa-reads-billing-asks-person")
@layer("project")
@ask("person")
forbid (
  principal in Workspace::"qa",
  action == Action::"read",
  resource in Workspace::"billing"
)
when { context.attended };   // unattended: no one to ask, so nothing permits it and the
                             // default deny decides
```

The chat is `Chat::"…"` with `Workspace::"qa"` as its parent. The file is `File::"…"` with its
workspace as its parent. `in` follows that hierarchy.

### Rego (regorus)

```rego
package purlis.policy
import rego.v1

hits contains {"rule": "qa-may-not-read-billing", "effect": "deny"} if {
  input.principal.workspace == "qa"
  input.action == "read"
  input.resource.workspace == "billing"
}

# Variant
hits contains {"rule": "qa-reads-billing-asks-person", "effect": "ask", "whom": "person"} if {
  input.principal.workspace == "qa"
  input.action == "read"
  input.resource.workspace == "billing"
  input.context.attended
}

# The combining rule is ours to write: deny > ask > allow > default deny.
default decision := {"effect": "deny", "rules": ["default"]}
decision := {"effect": "deny", "rules": [h.rule | some h in hits; h.effect == "deny"]} if {
  some h in hits; h.effect == "deny"
} else := {"effect": "ask", "rules": [h.rule | some h in hits; h.effect == "ask"]} if {
  some h in hits; h.effect == "ask"
} else := {"effect": "allow", "rules": [h.rule | some h in hits; h.effect == "allow"]} if {
  some h in hits; h.effect == "allow"
}
```

### TOML with a purlis evaluator

```toml
[[rule]]
id = "qa-may-not-read-billing"
layer = "project"
who = { chats_of = "qa" }
does = ["read"]
to = { workspace = "billing" }
effect = "deny"

# Variant
[[rule]]
id = "qa-reads-billing-asks-person"
layer = "project"
who = { chats_of = "qa" }
does = ["read"]
to = { workspace = "billing" }
when = { attended = true }
effect = { ask = "person" }
```

## Benchmark

**Set-up** [measured]:

- A scratch crate outside the repo, built with `--release`:
  - `cedar-policy = "=4.14.1"`, default features;
  - `regorus = "=0.15.0"`, default features, using the `Engine` interpreter, not the RVM;
  - a plain-Rust linear scan standing in for the TOML evaluator.
- **199 rules shaped like Q307**, over 50 workspaces (48 plus `qa` and `billing`):
  - per workspace, own-workspace read, write and run;
  - per workspace, an ask before an unsandboxed connect;
  - per workspace, an attended read of the next workspace;
  - 50 sandboxed host allows;
  - Q307's forbid, an ask variant, and a global "no unsandboxed, unattended run" forbid.
- **One decision per simulated hook call.** Each call:
  1. builds its request from scratch: Cedar entities (chat → workspace, file → workspace) and
     context, or Rego's JSON input;
  2. decides;
  3. reads the deciding rule IDs and their `@ask` annotations, or Rego's `hits`.
- The policy set is parsed once, as the host would hold it.
- 5,000 deterministic pseudo-random calls: 559 allow, 639 ask and 3,802 deny. **All three
  engines agreed on every one** before any timing.
- Then 2,000 warm-up calls and 20,000 timed calls each, single-threaded.
- **Machine:** Apple silicon (arm64), 12 cores, macOS 27.2. Load averages were 35.6, 32.9,
  31.7 and 29.6 across the runs, which is heavily oversubscribed, with many other chats
  building. The numbers are pessimistic.

| Engine | p50 | p99 | max | Policy load |
|---|---|---|---|---|
| Cedar 4.14.1 | 93.6–94.8 µs | 128.0–247.3 µs | 0.4–42.3 ms | 2.8–3.8 ms |
| regorus 0.15.0 (interpreter) | 169.0–169.6 µs | 248.8–267.6 µs | 3.1–24.1 ms | 2.4–3.1 ms |
| Plain Rust scan (TOML stand-in) | 0.5 µs | 0.8 µs | 11–22 µs | — |

The ranges cover three runs. The max column is scheduler noise at that load.

- **Building the request** (Cedar entities, context and request, with no evaluation) costs
  **6.4 µs p50 and 7.0 µs p99**. Nearly all of Cedar's cost is evaluating 199 policies: about
  0.45 µs per policy [measured, then inference].
- That is about 20× the paper's medians, which used datastores with far fewer policies on an
  idle EC2 m5.4xlarge [inference].
- If cost stays linear, 2,000 rules would be about 1 ms at this load, which is still within
  budget. Measure that before relying on it [inference].
- Policy load is about 3 ms. That is irrelevant while the host holds the set, because hooks
  reach a long-lived host over `hooks.sock` [repo, `crates/host/tests/it/a_held_projects_hooks_are_heard_in_the_host.rs`].
  It would matter only if `purlis hook` evaluated in its own process.

## Open risks

1. **Ask and lock are purlis conventions.** Cedar ignores annotations when it evaluates, so
   "deny > ask > allow" and the permit-lock check live in purlis and need their own tests. SymCC
   does not know them either: each analysis has to be phrased over the right policy subset
   (see (a)).
2. **Paths.** `like` has only `*`, which matches across `/`. Segment-aware globs mean modelling
   paths as an entity hierarchy (File in Dir in Workspace), or normalising them in purlis before
   the request. This needs a design ticket.
3. **The sandbox needs the question asked the other way round.** Seatbelt and bubblewrap
   profiles need "which paths may this chat read?", not one yes or no. That is partial or typed
   partial evaluation (`tpe`), which is **experimental** in cedar-policy 4.14.1 (crate
   features). The alternative is compiling the profile from the policy's resource scopes in
   purlis. This is unresolved, and it may be the largest piece of work.
4. **Skip on error.** A policy that errors is skipped. Purlis must validate strictly against a
   schema at load and refuse a start on any error, never run with a partial set.
5. **Default deny names no rule.** Purlis reports it as `default`, or the shipped policy ends
   in a named catch-all.
6. **Scale.** Evaluation looked linear in policy count here. The cost at an organisation's
   policy size, and at the spec's scale target, is not measured. The ≤ 50 ms hook budget
   leaves room, and the "few milliseconds" decision budget is still "not yet specified" in
   #6969.
7. **SymCC needs `cvc5`.** It is an external solver binary (and the crate pulls in tokio), so it
   belongs in CI or behind an optional authoring tool, never in the shipped hook path. Whether
   CI may download cvc5 is a decision for M75.
8. **Upgrades.** cedar-policy 4.x ships about monthly. Pin it, and read its CHANGELOG on each
   bump. Version 4 has been semver-stable since 2024-09.
9. **Not verified this session:**
   - Cedar's CNCF maturity level. `cncf.io` is not an allowed host.
   - `cedarpolicy.com` (bare) is not allowed: `www.cedarpolicy.com` redirects there, so
     docs.cedarpolicy.com was read instead.
   - The Cedar Lean spec repository (`cedar-policy/cedar-spec`).
   - regorus's third-party Cedar reproduction.
   - Whether regorus 0.15.0 has any public producer of `ExplanationReport`.
   - OPA's v1.2.0 versioned docs, which the sandbox refused.
