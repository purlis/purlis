# Amendment, 2026-11-22: the route carries a chat's CI and land reads (#7358)

The first live use of one login (#7302) showed the route refusing work a chat does on its own
repo every day: `tools/land.mjs` could not read the rules that apply to a branch, a chat could
not read a merge queue, a CI job's log answered 502, and a failed run could not be rerun. This
amendment adds each of them for **the chat's recorded repos only**. Nothing in D3d and D3e's lists of what
stays refused changes: merges (and the enqueue of one), reviews and approvals that count toward a
merge, releases and tags, repository administration, and every write to a rule.

## What is added

1. **Rules are a read.** `GET repos/{o}/{r}/rulesets`, `rulesets/{id}` and
   `rules/branches/{branch}` are answered. **They include the organisation rulesets that apply to
   the repo** (inherited ones), not only the repo's own: the chat reads the rules its branch is
   under, wherever they were set. They were refused with the rest of the names in the
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
   (`productionresultssa<1 to 5 digits>.blob.core.windows.net`, `*.actions.githubusercontent.com`
   except `token.` and `vstoken.`). The
   listener follows it **once, by a GET, and with no credential**, exactly as it follows a
   release asset. The pre-signed address carries its own authority, and only the headers
   `accept`, `range`, `if-none-match` and `if-modified-since` are sent on. The follow is made only for a credentialed read of the API (`Class::ApiRead`), and never after a
   write or a second redirect. The match is
   exact: the digits and the `productionresultssa` prefix, or one label under
   `actions.githubusercontent.com`. **A bare `*.blob.core.windows.net` is never matched**, as any
   storage account is anyone's. These hosts are followed **only for a chat on its unix socket**;
   they are on no tunnel list, so the chat's own egress still refuses them, and a client that goes
   through `HTTPS_PROXY` (curl, a browser) is told 502 as before. Widening the tunnel is a larger
   decision (D3g, D3i) and is not made here.
4. **A run is rerun.** `POST actions/runs/{id}/rerun` and `rerun-failed-jobs`, on a recorded
   repo, with no query and a body that is empty or `{}`: a body that names anything (such as
   `enable_debug_logging`) is refused as malformed. **`approve` is not added**: it stays refused
   as an approval (`Refusal::Approval`).

## Why a rerun is not a held act (D8), and why `approve` is refused

A held act is a request the allowlist refuses and that the person may press to make: a merge, a
tag, a release, a fork, a push that changes CI files. Each changes what the repo _is_ or _ships_.
A rerun does neither: it runs the workflow files at that run's commit, which a chat can already
cause by pushing its own branch (D3f). The class (`Class::CiWrite`) is its own, so the audit says
what was done.

**What a rerun is, said plainly.** A rerun **replays a past run with its secrets**. That includes
an old deploy or release run on `main`: a chat can rerun it and so **redeploy a stale SHA**. An
environment's required approvals still hold, since a rerun waits on them as the first run did, but
nothing else here stops it. The person who does not want that signs the chat out of the forge.

**`approve` is refused.** It releases a run held for a fork's or a first-time contributor's
commit, and an outsider's fork pull request can word a task so that a chat approves its own run:
that runs a stranger's code, on self-hosted runners where the repo has them, can chain into a
`workflow_run` with secrets, and spends Actions minutes. It is the person's to press, as a merge
under enforce is. Routing it through the held-act hand-off needs a new kind of ask and its
executor in the app, which is more than this amendment; until then it is refused, and a
follow-up may hand it off.

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
