# Amendment 2026-11-27: forge operations under policy, the fixed forge floor

Amends [ADR 0128](../0128-your-sign-in-brokered-one-approach-for-every-forge.md), D3d (the REST
allowlist), D3e (the GraphQL resolver, as far as it names mutations) and D8 (held acts). It
records the person's rulings of 2026-11-27 on "all forge concepts we support need to be in
policies as well", given after three critiques (security, design fit, UX and scope), and their
second round of the same day, given after a review of the first draft. It also amends the
amendment of 2026-11-22, "the route honours `guards.merge`" (§7). Docs only.

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

Every forge operation purlis supports, except merges, releases and tags (§7), is one Cedar action
in the family `guards.forge`, with the recorded `Repo` as its resource. A branch update through
the API is `guards.git`'s too (§7). Each is named forge-neutral, as `object.act`: `issue.create`,
`comment.create`, `pull.create`, `milestone.create`, `label.create`, `ci.rerun`, `ci.cancel`.
GitHub and GitLab requests for the same act map to the same action.

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
- **Each body field is its own operation.** A body key that performs another listed operation
  belongs to that operation, not to the row it arrives on: `assignees` in an issue's update is
  `issue.assign`, `labels` is `issue.label`. The request is decided under each action it spans,
  and every one must allow (§2).

### 2. One mapping table

The route holds one table. Each row is (forge kind, method, exact path pattern, allowed body-key
set) and gives exactly one target: a (family, action), or **held, no action** for a fork, which
stays the person's press (D7, D8.6) and is decided by no policy. REST routes and GraphQL mutations
are both rows: a GraphQL row is (forge kind, mutation name, allowed input-field set).

- **A row may fix one named body key's value** from a closed list, when the forge puts two acts
  on one route: `milestone.update` and `milestone.close` are one `PATCH` told apart by `state`,
  and the review actions are told apart by `event` (§9). A key so fixed is part of the row's
  pattern, and the no-overlap test covers it.
- **A body key that belongs to another operation** (§1) adds that operation's action to the
  request. Each row lists, beside its own keys, the keys it carries for other actions and the
  action each one maps to. The request is decided under every action it spans, and one refusal
  refuses it.
- **A GraphQL document** is decided field by field. Each top-level mutation field, with its
  aliases and fragments resolved, is mapped and decided on its own, and one refusal refuses the
  document. A document that holds a permit-required (§5) or held act must be exactly one mutation
  with one field, or it is refused.

The route decides in this order:

1. **parse** the request;
2. **canonicalise** it (D3d's rules: one percent-decode, no `.` or `..`, no prefix match);
3. **map** it to a row. No row means `NotOnList`, and Cedar is never asked;
4. **structural checks**: every check of today's `check_body` (credentials, quick actions, labels
   by id, line breaks), no query on a write included, head ownership, and the repo tie (the
   recorded owner/name and numeric id; every GraphQL id tied to a recorded repo);
5. **Cedar**, for each action the request spans, with the recorded `Repo` and the host-supplied
   context (`context.owner` included: ADR 0125, amended 2026-11-27). A held row is not asked of
   Cedar: it goes to the person's press.

On a forge action, `context.owner` is the relation to the actor (ADR 0125, amended 2026-11-27,
§5: `"self"`, `"task"`, `"asker"`, `"other"`, `"none"`) of the chat the forge route recorded
creating the branch the act writes or runs on (a pull request's head, a run's head branch). Who created a non-branch object (a comment, an issue, a
milestone) is a separate optional field, `context.made_by`, with the same values. It is reserved
in the version-1 schema now and filled in M81, from the record of the objects a chat created.

The table is tested for:

- **no overlap:** no request matches two rows;
- **totality:** every row names an action the schema declares or is held with no action, and
  every forge action in the schema has at least one row, except the actions §5 marks as not yet
  routed;
- **closed bodies:** an unknown body key on a mapped route is refused.

### 3. The mapping oracle

The table is checked against the forge's own description of its API:

- **GitHub:** the pinned OpenAPI description (`github/rest-api-description`) and the pinned
  GraphQL schema. Every operation in them is classified as a row, held with no action (a fork),
  a read, the floor (§6), or not routed.
  CI fails if one is unclassified, so a new GitHub operation is a decision, not a silent refusal
  or a silent pass.
- **GitLab:** a hand-kept list, checked against recorded `glab` traffic, because GitLab publishes
  no complete machine description of its REST API.

### 4. The API version is pinned

The route pins `X-GitHub-Api-Version` to the version the oracle's description was taken at, and
replaces any value the chat sent. A request is never read under one version's shape and answered
under another's. Today the header passes through untouched.

The pinned version is `2022-11-28`, the one purlis's own client sends. A GHES host gets the same
header: the route never sends one version to github.com and another to GHES. A GHES release that
does not take it, or whose description lacks or reshapes an operation, is a gap cell of M81's
parity work, not a second version.

### 5. The permit-required class

An action annotated `@class("permit-required")` is allowed only by a permit, or a granted Ask,
that names that action explicitly in its scope.

- The catch-all `policy-everything-else` never reaches it.
- No mode (`off`, `log`) and no template turns it on.
- The host checks that each determining permit names the action, by `==` or in an explicit list.
  The action's own UID must appear literally: a permit over the whole family or every action does
  not count, and neither does a list holding a group the action belongs to.
- An Organisation permit for it is still a loosening, and waits for the person's press on this
  machine, as every loosening does (ADR 0125, amended 2026-11-27, §3).
- For an askable action, the outcome with no permit is an **Ask the host raises**. It is not an
  `@ask` forbid, so neither the `asks` mode nor the family's mode lifts or silences it.
- An action's membership of the class never changes within version 1 of the schema. It can
  leave the class only by a later amendment.

This is the chat-to-chat rulings' class, "new abilities by explicit permit only" (ADR 0125,
amended 2026-11-27), applied to the forge. Every operation that is refused today and opened by
this amendment or by M81 is in it. Today's operations are not, and keep today's outcome, with
one exception the person ruled: assignee changes (`issue.assign`, `pull.assign`), which are
forwarded today and join the class (§8).

### 6. The fixed forge floor

No action exists for the acts below. They have no row in the table and no action in the schema,
so no level, template, mode or permit can allow them, the Organisation's included. They are
outside the policy's vocabulary rather than fixed rules inside it, so they add nothing to M75
spec §2.5's fixed things or to ADR 0125's fixed floor (amended 2026-11-27, §2). The inventory
mapping (spec §15) maps each one to "floor: no action".

Every name on today's `ADMIN`, `RULESET` and `APPROVAL` lists (`rest.rs`) belongs to a floor
entry and is refused for every method, reads included, except the rule reads of #7358 and
reading a pull request's reviews and its approval state, which stay allowed: only review and
approval writes are on the floor.
`dispatches` belongs to `repository_dispatch`'s entry: `workflow_dispatch`
(`actions/workflows/{file}/dispatches`) leaves it only by its own M81 row.

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
- environments, their protection rules and their branch policies;
- pipeline schedules (`pipeline_schedules`), and GitLab's `POST pipeline` with variables;
- collaborators, members, invitations, teams and permissions;
- deleting, renaming or transferring the repo, importing into it, changing its visibility, and
  its settings (`settings`, `autolinks`, `vulnerability-alerts`, `security-and-analysis`,
  `import`, `rename`, `transfer`);
- commits and ref writes through the API: `git/refs`, `git/commits`, `git/trees`, `git/blobs`,
  `contents` and GitLab's `repository/files` and `repository/commits` writes, `repository/tags`
  writes other than a new tag's creation (below), and `merge-upstream`. Code moves only by push (D3f, D8.3), and a
  branch is deleted only by push, so `DELETE git/refs/heads/…` and GitLab's
  `DELETE repository/branches/…` are here too.
  Two exceptions: a new tag's creation (`git/refs` naming `refs/tags/…`, `git/tags`, GitLab's
  `repository/tags`), which is `guards.release`'s and held, and a pull request's branch update,
  which is `guards.git`'s `update` (§7);
- **enabling auto-merge:** GitHub's `enablePullRequestAutoMerge` and `auto_merge`, GitLab's
  `merge_when_pipeline_succeeds=true` in a body or a query, `auto_merge`, and a
  `mergeRequestAccept` with `strategy` or `autoMergeStrategy`. Disabling auto-merge is a merge row
  (§7);
- a branch merge with no pull request (`POST merges`, `mergeBranch`), and a merge command in
  text;
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
  makes, with `context.owner` (M75-26). They are not `guards.forge` actions. Its rows are a pull
  request's merge, disabling auto-merge (`disablePullRequestAutoMerge`, GitLab's
  `cancel_merge_when_pipeline_succeeds`), the merge queue's enqueue and dequeue, and
  `mergeRequestAccept` with no auto-merge strategy.
- **This amends the amendment of 2026-11-22, "the route honours `guards.merge`".** Enabling
  auto-merge leaves its "what opens": `enablePullRequestAutoMerge` and GitLab's
  `merge_when_pipeline_succeeds=true`, in a body or as the query's one parameter, are on the floor
  (§6) under every mode. `merge_when_pipeline_succeeds=false` stays a plain merge.
- Release and tag creation through the API (a release, and a new tag: never a moved one) are
  decided by `guards.release`. They stay held for the person's press (D8.2) whatever
  `guards.release` says: `guards.release` can tighten the route's hold to a refusal, never lift
  it, and `guards.release = "off"` never opens API tag or release creation (M75-27). The equality
  cell between the hook and the route is therefore stated for `enforce` only.
- A draft release's edit (`release.edit_draft`, M81) is `guards.release`'s too, in the
  permit-required class (§5).
- **A pull request's branch update** (GitHub's `PUT pulls/{n}/update-branch` and
  `updatePullRequestBranch`, GitLab's `PUT merge_requests/{iid}/rebase`) writes to the pull
  request's head, so it is `guards.git`'s `update` action on that `Branch`, with `context.owner`
  read from the forge route's record of the head (`branch_owner`), as `push` reads it: an API
  update has no branch folder, so no claim. This amends ADR 0125 (amended 2026-11-27, §5), where
  `update` reads the folder claim: that holds for a local update alone. The head-ownership check stays structural (§2, step 4). It is not a `guards.forge`
  action.
- No other ref-writing API route maps to any action (§6).

### 8. Tracker writes

Milestones, repo labels, assignees and reactions are tracker writes:

- under `open`, `watch`, `hold` and `team`, an Ask by default (§5), and a named permit makes the
  answer standing;
- under `strict`, a named permit for that operation stops the asking; without one, an Ask every
  time;
- M75 opens milestones (`milestone.create`, `milestone.update`, `milestone.close`) and repo labels
  (`label.create`, `label.update`, `label.delete`), as the first permit-required pair;
- **assignee changes are an Ask on both forges.** `issue.assign` and `pull.assign` are in the
  permit-required class in M75, wherever the assignee arrives: a body key on today's routes
  (`assignees`, `assignee_ids`) or GitLab's `mergeRequestSetAssignees`. **This is a named
  behaviour change:** `mergeRequestSetAssignees`, and an assignee key on an issue or merge request
  write, are forwarded today and become an Ask. GitHub's own assignee routes
  (`issues/{n}/assignees`) are M81's;
- reactions and lock are M81's;
- the offer comes in M81 (#7796), after two identical answers to the same tracker-write Ask.
  It offers the person's standing answer at the You level on this machine, never a proposed
  project rule. Only tracker-write Asks offer one; every other Ask offers none, and so does M75's.

### 9. Reviews and approvals: D3d amended to match the code

D3d's "review with COMMENT or REQUEST_CHANGES" is withdrawn. As built, every review write is
refused (`APPROVAL` names `reviews`), and that is what D3d now says. Review comments
(`pulls/{n}/comments`) and their replies stay allowed, as built.

Comment actions are forge-neutral: `comment.create` and `comment.update` cover every comment
route, an issue's, a pull request's, a diff comment and a reply on either forge. A diff comment is
not an action of its own: what sets it apart (`commit_id`, `path`, `line`) is its row's key set,
and no ruling asks a policy to tell it apart.

COMMENT and REQUEST_CHANGES reviews come back in M81 as permit-required actions
(`pull.review.comment`, `pull.review.request_changes`). Each allows exactly that `event` and binds
to a `commit_id` that is the pull request's head. APPROVE stays on the floor (§6).

D3d's branch line now reads: "branches, write: only by push (D3f), a branch deleted too. No API
ref write is on the list; a pull request's branch update is `guards.git`'s `update`." Its label
line now reads: "an issue's or pull request's labels, set and removed by name; repo labels,
read." Its line "issues, labels, assignees, milestones (read), comments" now reads: "issues and
comments; labels, assignees and milestones, read. An assignee is set only as `issue.assign` or
`pull.assign`, which ask (§8)."

### 10. Refusal text

A forge refusal is one of two kinds, and says which:

- **No action:** "purlis doesn't carry this request (`<METHOD> <canonical path>` on
  `<host>/<owner>/<repo>`), so it was refused. No policy can allow it." For a request no row
  matches, each path segment outside the route vocabulary is shown as `{…}`, the query is
  dropped, and the text is capped and drawn inert. A floor act also names
  its entry and that it's the person's to do on the forge. Neither is askable.
- **A known act the policy doesn't allow:** "`<action>` on `<host>/<owner>/<repo>` isn't allowed
  by `<rule @id>` (`<level>`, `<owner>`)." An askable act raises an Ask in the person's Asks in
  place of a bare refusal.
- **A permit-required act with no permit** has no rule `@id` to name: "`<action>` on
  `<host>/<owner>/<repo>` is allowed only by a rule that names it, and no rule does." An askable
  one raises an Ask in its place (§5).
- **Under a Broken or Waiting acceptance**, a known act refused because the policy isn't accepted
  says so: "`<action>` on `<host>/<owner>/<repo>` was refused because this project's policy isn't
  accepted on this machine. Accept it in purlis's window." It names no family to loosen (ADR
  0130, §7; #7717).

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
  closed bodies with each body field decided as its own operation, the removal of
  `ForgeCtx.verb` and the reservation of `context.made_by`, the permit-required class, the floor
  and its proof, today's operations as actions, milestones and labels as the first
  permit-required pair, assignee changes as an Ask, the oracle and the coverage cells for each
  action.
- **M81 "Forge operations under policy"** (milestone 60) takes the rest:
  - CI cancel, job retry and `workflow_dispatch`;
  - COMMENT and REQUEST_CHANGES reviews;
  - release draft edits;
  - Projects v2, discussions, wikis and snippets;
  - own-branch delete, by push;
  - GitHub's assignee routes, reactions and lock;
  - security: code-scanning and Dependabot reads, and their writes that dismiss nothing;
  - deployments: a deployment's and its status's creation, or a floor entry;
  - deleting a chat's own comment, and the record of the objects a chat created
    (`context.made_by`);
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
  M81's record (`context.made_by`).
