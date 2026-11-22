# Amendment 2026-11-22: the route honours `guards.merge`

Amends [ADR 0128](../0128-your-sign-in-brokered-one-approach-for-every-forge.md), D2 (a chat never
merges) and D8.1 (a merge is a held act).

## What was wrong

A person set `[policy] guards.merge = "log"` and accepted it, meaning "chats may merge". The merge
guard in the hook honoured it (`crates/purlis-cli/src/guard.rs`): `off` is not asked, `log` is let
through and logged. The one-login forge route did not: it refused every merge whatever the policy,
by REST (`PUT repos/{o}/{r}/pulls/{n}/merge`), by GraphQL (`mergePullRequest`, auto-merge, the merge
queue) and by GitLab, and handed it to the held-act hand-off. So `gh pr merge` was refused by the
route in the chat the hook had let through.

## Decision

The route asks the project's `guards.merge`, **live on each request**, as `forge/pin.rs` reads
`Locks` on each ask, and keeps no copy. The one source of truth is `purlis_core::enforcement`
(`enforcement::mode`): the stricter of the committed file and what the person accepted, never below
an administrator's lock, and `enforce` wherever the policy cannot be read or the process is not the
app. The route adds no reading of its own (`forge::mergestance`).

- **`enforce`**: unchanged. The merge is a held act (D8.1); the person is asked.
- **`log`**: the merge is forwarded and one line is recorded in the chat's enforcement log, family
  `guards.merge`, rule `merge-hold`, as the guard's `log_verdict` does. The line names the chat and
  the repo as `host/owner/repo` in the log's one free field, never a body, a path or a number.
- **`off`**: forwarded, no line.

**What opens, and nothing else** (`ChatPolicy::decide_merging`):

- REST: GitHub `PUT pulls/{n}/merge`; GitLab `PUT merge_requests/{iid}/merge` (with its
  `merge_when_pipeline_succeeds` in the body) and `POST …/cancel_merge_when_pipeline_succeeds`.
- GraphQL, GitHub: `mergePullRequest`, `enablePullRequestAutoMerge`, `disablePullRequestAutoMerge`,
  `enqueuePullRequest` (the merge queue) and `dequeuePullRequest`. GitLab: `mergeRequestAccept`.
  `gh pr merge`, with `--auto` and the queue, speaks these. `enqueuePullRequest` and
  `dequeuePullRequest` are now named as merges, so they are held under `enforce` as the others are,
  where before they were an unknown mutation.

Each is decided by the same checks as any write: a recorded repo of this chat's own run, a body free
of merge commands, no query, and, in GraphQL, every id tied to a recorded repo.

## What stays as it was

- Only a recorded repo of the chat's own run, and a valid sentinel. A merge of another repo, or with
  none, is refused or held as before; the stance is asked only after the sentinel is proved.
- Administration, tags and releases, rulesets and branch protection, approvals, a branch merge
  (`POST merges`, `mergeBranch`) and a merge flag in a query string (`?auto_merge=true`,
  `?merge_when_pipeline_succeeds=true`) stay refused. The last is refused as a malformed request
  where the stance opens the route: a client sends the flag in the body.
- An administrator's lock wins (`Locks::enforcement_floor`), and so does a pin (D3g), asked before
  the stance.
- **Fail closed.** A policy that cannot be read is `enforce`. A merge that `log` or `off` would let
  through but whose request is anything the checks above refuse is held or refused as under
  `enforce`.

## Known limit

A GraphQL mutation by object id (`mergePullRequest(input:{pullRequestId})`, which `gh pr merge`
sends) is tied to a recorded repo by an `ObjectRepo`. The app's route is built with `NoObjects`,
so every by-id mutation, `closePullRequest` included, is refused as another repo until a resolver
is wired; REST and GitLab merges and a mutation that names its repository are not affected. This
amendment does not change that.

**Tier** (ADR 0069): no new store. The log line is the enforcement log's own.
