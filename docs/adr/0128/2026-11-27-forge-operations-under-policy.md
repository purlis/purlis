# Amendment 2026-11-27: forge operations under policy, the fixed forge floor

Amends [ADR 0128](../0128-your-sign-in-brokered-one-approach-for-every-forge.md), D3d (the REST
allowlist), D3e (the GraphQL resolver, as far as it names mutations) and D8 (held acts). It
records the person's rulings of 2026-11-27 on "all forge concepts we support need to be in
policies as well", given after three critiques (security, design fit, UX and scope). Docs only.

## What was wrong

A chat asked the forge route to create a milestone and was refused `NotOnList`. Nothing in the
project's policy could have let it through, and the refusal did not say whether the act was
unknown to purlis or known and not allowed.

The route reads one policy family today: `guards.merge` (`forge/mergestance.rs`), plus an
administrator's pin (`Refusal::Pinned`). Everything else is hard-coded in
`crates/purlis-core/src/forge/proxy/`:

- the writes are `write_class` (`rest.rs`) and `mutation_allowed` (`graphql.rs`), and a route
  missing from them is `NotOnList`;
- `ADMIN`, `MERGE`, `APPROVAL`, `RELEASE` and `RULESET` only name the reason for a refusal; they
  never let a request through.

M75 adds `guards.forge` with the actions `call` and `mint`. It types the act as
`ForgeCtx.verb: String`, and the M75 schema defines no list of verbs. A string verb can't be
checked by the Cedar validator, can't be counted by the coverage table, and lets a policy author
write a verb that no route will ever send. Under M75 spec §2.5 ("a project may loosen
anything"), the hard-coded refusals would become loosenable, unless they are declared fixed.

D3d also disagrees with the code in three places:

- it allows a review with COMMENT or REQUEST_CHANGES, but `APPROVAL` in `rest.rs` names
  `reviews`, so every review write is refused;
- it lists "branches, write: create a ref under a name the chat created", but no API ref write
  is on `write_class`. A branch is made by a push alone (D3f);
- it lists labels with the reads only. The code allows setting and removing an issue's labels by
  name (`POST issues/{n}/labels`, `DELETE issues/{n}/labels/{name}`). Repo labels themselves
  (create, edit, delete) are refused.

## Decision

### 1. A typed list of forge operations, one Cedar action each

Every forge operation purlis supports is one Cedar action in the family `guards.forge`, with the
recorded `Repo` as its resource. Each is named forge-neutral, as `object.act`: `issue.create`,
`issue.comment`, `pull.create`, `pull.comment`, `milestone.create`, `label.create`, `ci.rerun`,
`ci.cancel`. GitHub and GitLab requests for the same act map to the same action.

- **An exception to "actions are lowercase verbs"** (M75 schema header, Q311.7), for this family
  alone. Its actions are quoted dotted names (`action "milestone.create" in ["guards.forge"]`), as
  the family names already are. Every other family keeps the rule.
- **`ForgeCtx.verb` is removed before schema v1 freezes.** The action is the verb. The action
  `call` goes with it.
- **An unknown request has no action, and Cedar is never asked about it.** Default-deny stays a
  property of the code, not of the policy: no policy, template or mode can reach a request that
  no row maps.
- **Each operation has an exact request shape:** the method, the path pattern and the set of body
  keys it allows. A body key outside the set is refused, before Cedar.

### 2. One mapping table

The route holds one table. Each row is (forge kind, method, exact path pattern, allowed body-key
set) and gives exactly one (family, action). REST routes and GraphQL mutations are both rows: a
GraphQL row is (forge kind, mutation name, allowed input-field set).

The route decides in this order:

1. **parse** the request;
2. **canonicalise** it (D3d's rules: one percent-decode, no `.` or `..`, no prefix match);
3. **map** it to a row. No row means `NotOnList`, and Cedar is never asked;
4. **structural checks**: `check_body` (credentials, quick actions, labels by id, line breaks),
   head ownership, and the repo tie (the recorded owner/name and numeric id; every GraphQL id tied
   to a recorded repo);
5. **Cedar**, for the row's action, with the recorded `Repo` and the host-supplied context
   (`context.owner` included: ADR 0125, amended 2026-11-27).

The table is tested for:

- **no overlap:** no request matches two rows;
- **totality:** every row names an action the schema declares, and every forge action in the
  schema has at least one row, except the actions §5 marks as not yet routed;
- **closed bodies:** an unknown body key on a mapped route is refused.

### 3. The mapping oracle

The table is checked against the forge's own description of its API:

- **GitHub:** the pinned OpenAPI description (`github/rest-api-description`) and the pinned
  GraphQL schema. Every operation in them is classified as a row, the floor (§6), or not routed.
  CI fails if one is unclassified, so a new GitHub operation is a decision, not a silent refusal
  or a silent pass.
- **GitLab:** a hand-kept list, checked against recorded `glab` traffic, because GitLab publishes
  no complete machine description of its REST API.

### 4. The API version is pinned

The route pins `X-GitHub-Api-Version` to the version the oracle's description was taken at, and
replaces any value the chat sent. A request is never read under one version's shape and answered
under another's. Today the header passes through untouched.

### 5. The permit-required class

An action annotated `@class("permit-required")` is allowed only by a permit, or a granted Ask,
that names that action explicitly in its scope.

- The catch-all `policy-everything-else` never reaches it.
- No mode (`off`, `log`) and no template turns it on.
- The host checks that each determining permit names the action, by `==` or in an explicit list.
  A permit over the whole family or every action does not count.

This is the chat-to-chat rulings' class, "new abilities by explicit permit only" (ADR 0125,
amended 2026-11-27), applied to the forge. Every operation that is refused today and opened by
this amendment or by M81 is in it. Today's operations are not: they keep today's outcome.

### 6. The fixed forge floor

No action exists for the acts below. They have no row in the table and no action in the schema,
so no level, template, mode or permit can allow them, the Organisation's included. They are
outside the policy's vocabulary rather than fixed rules inside it, so they don't add to M75
spec §2.5's three fixed things. The inventory mapping (spec §15) maps each one to "floor: no
action".

- webhooks and integrations (`hooks`, `integrations`, `services`, `installation`);
- deploy keys and tokens (`keys`, `deploy_keys`, `deploy_tokens`, `access_tokens`);
- secrets and variables, their reads included;
- secret-scanning alerts, their reads included;
- branch protection, rulesets, push rules and approval rules;
- approving or dismissing a review, and approving one's own pull request or merge request. A
  chat never approves a pull request;
- commit statuses, check runs and attestations, as writes;
- dismissing a code-scanning, secret-scanning or Dependabot alert;
- approving a CI run or a deployment: `actions/runs/{id}/approve`, `pending_deployments`,
  `deployment_protection_rule`, and GitLab's `play`;
- collaborators, members, invitations, teams and permissions;
- deleting, renaming or transferring the repo, changing its visibility, and its settings;
- commits and ref writes through the API: `git/refs`, `git/commits`, `git/trees`, `git/blobs`,
  `git/tags`, `contents` and GitLab's `repository/files` and `repository/commits` writes, and
  `merge-upstream`. Code moves only by push (D3f, D8.3);
- auto-merge, a branch merge with no pull request (`POST merges`, `mergeBranch`), and a merge
  command in text;
- credentials in a request, and the OAuth and device flows;
- gists, packages and registries, runners, and Pages;
- replacing a release asset, and moving or deleting a tag;
- `repository_dispatch`, and GitLab's pipeline triggers, which carry a payload of the caller's
  choosing.

A test proves the floor under the most permissive policy that validates: every action permitted,
every family `off`, the `open` template. Every floor request is still refused, and Cedar is never
asked.

### 7. Merges, releases and tags are their own families

- The route's merges are decided by `guards.merge`'s `merge` action, the same decision the hook
  makes, with `context.owner` (M75-26). They are not `guards.forge` actions.
- Release and tag creation through the API are decided by `guards.release`. They stay held for
  the person's press (D8.2) whatever `guards.release` says: `guards.release = "off"` never opens
  API tag or release creation (M75-27).
- No ref-writing API route maps to a forge action (§6).

### 8. Tracker writes

Milestones, repo labels, assignees and reactions are tracker writes:

- under `watch` and `team`, an Ask by default. The Ask offers a rule, and a named permit makes the
  answer standing;
- under `strict`, an Ask every time;
- M75 opens milestones (`milestone.create`, `milestone.update`, `milestone.close`) and repo labels
  (`label.create`, `label.update`, `label.delete`), as the first permit-required pair;
- assignees, reactions and lock are M81's. GitLab's `mergeRequestSetAssignees`, allowed today,
  maps to `pull.assign` and keeps today's outcome;
- M81 also offers a rule after two identical answers to the same forge Ask.

### 9. Reviews and approvals: D3d amended to match the code

D3d's "review with COMMENT or REQUEST_CHANGES" is withdrawn. As built, every review write is
refused (`APPROVAL` names `reviews`), and that is what D3d now says. Review comments
(`pulls/{n}/comments`) and their replies stay allowed, as built.

COMMENT and REQUEST_CHANGES reviews come back in M81 as permit-required actions
(`pull.review.comment`, `pull.review.request_changes`). Each allows exactly that `event` and binds
to a `commit_id` that is the pull request's head. APPROVE stays on the floor (§6).

D3d's branch line now reads: "branches, write: only by push (D3f). No API ref write is on the
list." Its label line now reads: "an issue's or pull request's labels, set and removed by name;
repo labels, read."

### 10. Refusal text

A forge refusal is one of two kinds, and says which:

- **No action:** "purlis doesn't carry this request (`<METHOD> <canonical path>` on
  `<host>/<owner>/<repo>`), so it was refused. No policy can allow it." A floor act also names
  its entry and that it's the person's to do on the forge. Neither is askable.
- **A known act the policy doesn't allow:** "`<action>` on `<host>/<owner>/<repo>` isn't allowed
  by `<rule @id>` (`<level>`, `<owner>`)." An askable act raises an Ask in the person's Asks in
  place of a bare refusal.

Each is counted under Settings › Policy, "Would have blocked". This is a bug fix on `main`, ahead
of M75. It uses today's classes: the deny-by-name lists give the floor's names, and the held acts
give today's Asks.

### 11. `mint`

`mint` has a live enforcement point: a run's start, when `secrets::brokered` hands a persona's
`mint = "github-app"` tool its token (`forge::apptoken::token_from_vault`). It stays in
`guards.forge` and is decided once per repo the token is minted over, with a cell of its own. It
is not a forge operation and has no row in the mapping table. Had no live point existed, it would
have been removed before v1.

## Phasing

- **M75** (milestone 55) takes the one-way doors: the typed list and the mapping table (M75-30),
  the removal of `ForgeCtx.verb`, the permit-required class, the floor and its proof, today's
  operations as actions, milestones and labels as the first permit-required pair, the oracle and
  the coverage cells for each action.
- **M81 "Forge operations under policy"** (milestone 60) takes the rest:
  - CI cancel, job retry and `workflow_dispatch`;
  - COMMENT and REQUEST_CHANGES reviews;
  - release draft edits;
  - Projects v2, discussions, wikis and snippets;
  - own-branch delete;
  - assignees, reactions and lock;
  - deleting a chat's own comment, and the record of the objects a chat created;
  - GitLab and GHES parity;
  - Ask less.
- **The refusal text** (§10) ships on `main` now.

## What stays as it was

- Every request is default-deny, decided on the canonical path, the recorded repo and a valid
  sentinel (D3b, D3d, D4).
- The held acts (merge, enqueue, a GitLab train entry, tag create, release create, fork, a
  CI-file push) are held for the person's press.
- A chat never holds the person's credential (D3), and the floor needs no credential to refuse.

## Rejected

- **A string verb** (`ForgeCtx.verb`): the validator can't check it, the coverage table can't
  count it, and a policy can name a verb that never arrives.
- **One action per HTTP route:** GitHub and GitLab would need two rules for one act, and a policy
  would depend on a forge's path shapes.
- **Loosenable admin acts** (spec §2.5 read literally): one Accept over a big diff would let a
  chat add a webhook or read a secret. The floor is the backstop for the cases the chat-to-chat
  rulings named: an Organisation loosening with no press, the seal gap on Linux and Windows, and
  `watch` being log-allow.
- **Entities for each forge object** (Milestone, Label, Workflow…): the act is what is decided.
  The object an act names is checked by the structural checks, and an object a chat created is
  M81's record (`context.owner` for forge objects).
