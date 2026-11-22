# Amendment, 2026-11-22: the route carries a chat's CI and land reads (#7358)

The first live use of one login (#7302) showed the route refusing work a chat does on its own
repo every day: `tools/land.mjs` could not read the rules that apply to a branch, a chat could
not read a merge queue, a CI job's log answered 502, and a failed run could not be rerun. This
amendment adds each of them for **the chat's recorded repos only**. Nothing in D3d and D3e's lists of what
stays refused changes: merges (and the enqueue of one), reviews and approvals that count toward a
merge, releases and tags, repository administration, and every write to a rule.

## What is added

1. **Rules are a read.** `GET repos/{o}/{r}/rulesets`, `rulesets/{id}` and
   `rules/branches/{branch}` are answered. They were refused with the rest of the names in the
   ruleset list (`Refusal::Ruleset`), for every method. Now only those three routes are read; any
   other route under a ruleset or a rule (`rulesets/{id}/history`, `rule-suites`, `rules`), every
   branch protection route, and every write, is still `Ruleset`. A branch with a `/` in its name
   is still refused as malformed (the target is read once), so `land.mjs`'s `main` is the
   case this serves.
2. **A merge queue is a read.** The GraphQL tables gain `PullRequest.isInMergeQueue`,
   `isMergeQueueEnabled` and `mergeQueueEntry`, `Repository.mergeQueue`, and `MergeQueue` and
   `MergeQueueEntry` types (state, position, enqueuer, the pull request). **`configuration` is
   not on the table.** `enqueuePullRequest`, `dequeuePullRequest` and auto-merge are still refused
   as merges; another task owns the guard that holds them.
3. **A run's logs and artifacts are downloaded.** GitHub answers `actions/jobs/{id}/logs`,
   `actions/runs/{id}/logs` and `actions/artifacts/{id}/zip` with a `302` to pre-signed storage
   (`productionresultssa<digits>.blob.core.windows.net`, `*.actions.githubusercontent.com`). The
   listener follows it **once, by a GET, and with no credential**, exactly as it follows a
   release asset. The pre-signed address carries its own authority, and only the headers
   `accept`, `range`, `if-none-match` and `if-modified-since` are sent on. The match is
   exact: the digits and the `productionresultssa` prefix, or one label under
   `actions.githubusercontent.com`. **A bare `*.blob.core.windows.net` is never matched**, as any
   storage account is anyone's. These hosts are followed **only for a chat on its unix socket**;
   they are on no tunnel list, so the chat's own egress still refuses them, and a client that goes
   through `HTTPS_PROXY` (curl, a browser) is told 502 as before. Widening the tunnel is a larger
   decision (D3g, D3i) and is not made here.
4. **A run is rerun or approved.** `POST actions/runs/{id}/rerun`, `rerun-failed-jobs` and
   `approve`, on a recorded repo, with no query. `approve` was refused by name as an approval
   (`Refusal::Approval`); it is **not a review**: it releases a workflow run held for a fork's or
   a first-time contributor's commit, and counts toward no required review and no merge.

## Why these are not held acts (D8)

A held act is a request the allowlist refuses and that the person may press to make: a merge, a
tag, a release, a fork, a push that changes CI files. Each changes what the repo _is_ or _ships_.
A rerun does neither: it runs the workflow files already at that commit, which a chat can already
cause by pushing its own branch (D3f). An approval starts a run the repo's own maintainers would
otherwise start by pressing a button in the same account's name; it ships nothing, and the merge
guards (D3d, D3e, D8.1) still decide whether anything lands. Asking the person for each rerun
would make CI unusable from a chat, which is the failure this amendment closes. The class
(`Class::CiWrite`) is its own, so the audit says what was done.

**What is left, said plainly.** An approved fork run executes that fork's workflow in the repo's
CI. GitHub withholds secrets from such runs and the chat cannot open the fork's pull request or
push to a fork (D3d, D3f), but the approval is the person's authority used by the chat. A person who
does not want a chat to use it signs the chat out of that forge; the route cannot tell a
run it should not approve from one it should.

## What is not changed

- `createPullRequest` and `createIssue` were already on the mutation list, tied to the chat's
  repo by `repositoryId`, with a head from a fork refused. They are covered by tests here and
  unchanged. What `gh pr create` and `gh issue create` also _read_ is the fixtures' list
  (`tests/fixtures/one-login-graphql/`); a field `gh` sends that is not on a table is still
  refused until it is read and added.
- An issue's timeline (`issues/{n}/timeline`) follows cross-references into other repos, as the
  list's comment says, and stays off.
- The refusal word for a GraphQL field not on the list is still `GraphqlOtherRepo`; renaming it
  and showing the route's audit are the rest of #7358.
