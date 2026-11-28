# How the app learns soon and cheaply that a branch's pull request merged

Research for #7996 (map #7990), 2026-11-28. Repo facts are read at `15eab6a312`. GitHub's
behaviour was **measured on `purlis/purlis` through `gh`** that day, so those facts are what the
forge itself answered. **The forge documentation sites could not be read from this chat:** the
sandbox refused `curl` to docs.github.com, and the forge proxy refused reading `github/docs`
(`OtherRepo`). So every fact that rests on documentation alone (GitLab, Gitea, webhooks, search
query limits) is listed under "Unverified" at the end and marked *(unverified)* where it is
used. None of them changes the recommendation.

## Answer

**Recommended: one conditional REST list of each repo's recently closed requests, sent by the
forge poller the app already has, plus a few prompts that cost nothing.** For each clone that
has branch folders, the app sends

```
GET /repos/{owner}/{repo}/pulls?state=closed&sort=updated&direction=desc&per_page=100
```

with `If-None-Match` (the native transport's ETag store does this already). It matches each
entry's `head.ref` and `head.repo.full_name` to the workspace's folders. A matched entry with
`merged_at` (or closed and not merged) hands that folder to the existing `finished` test
(`crates/purlis-core/src/finished.rs`), which keeps all its other conditions: no uncommitted
change, task ended, no chat holding it, and so on. GitLab gets the same shape (merge requests
listed by `updated_at`, *(unverified)*). A branch that predates the window of 100 is resolved
once, at startup or when the project opens, by the per-branch read purlis already sends
(`by_head`). On GitHub that backlog can instead be one GraphQL query per 100 branches.

- **Latency:** the poller's cadence (`crates/purlis-core/src/forge/poll.rs:34-39`): **60 s**
  while a Work view is open, **5 min** while the workspace is visible, **15 min** behind
  another window, and **nothing while every window is hidden** (caught up when one is shown).
  A merge the app makes itself (`purlis change land`, or a chat's merge through the forge
  proxy) is known **at once**, with no read.
- **Cost:** **one request per repo per interval, whatever the number of branches.** GitHub does
  not count an unchanged answer (a 304; measured below). At 5 min that is 12 requests an hour
  per repo, of which GitHub counts only those sent after something closed: at purlis's own
  pace (about 2.3 closings an hour) that is ≤ 3 counted an hour in practice and ≤ 12 at worst.
  GitLab counts every request, 304s included (`crates/purlis-core/src/forge/budget.rs`, the
  counting rule), so 12 an hour. purlis's allowance is 1,000 an hour per account
  (`budget.rs:57`).
- **Compared:** polling each of 150 branches costs 150 requests per interval (1,800 an hour at
  5 min), more than the whole allowance. Search has a separate limit of 30 requests a minute,
  and its query string holds only a few branch names. Webhooks cannot reach a desktop app
  without a public relay. `git fetch` alone cannot tell a merged request from a closed one.
  The table is in section 5.

**What has to be built:** the `Poller` exists but nothing in the app runs it yet (section 1.4).
It needs a watch per clone for this list, a consumer that runs the finished test on a match, and
a few prompts that make a watch due at once: window shown, Work view opened, a push or land
through purlis.

## 1. What purlis already has

### 1.1 When a finished folder is removed today

- A finished folder is taken away **only when its chat is closed and when the project is
  opened, and at no other time**: `app/src-tauri/src/dispatches.rs:39-42` (`tidy_closed`,
  `dispatchplace::tidy_at_open`, `crates/purlis-core/src/dispatchplace.rs:2358`). This is the
  gap #7996 is about. `purlis worktree prune` is a third, by hand
  (`crates/purlis-core/src/gitbroker.rs:385-412`, `crates/purlis-core/src/prune.rs:68`).
- What "finished" means is `crates/purlis-core/src/finished.rs:1-17` (ADR 0110). A folder's
  commits count as landed if they are on the base (`Commits::OnBase`), carried by a squash
  (`Commits::Carried`), or inside a merged request's head (`InMergedRequest`), or inside a closed
  request's head that is still on origin (`InClosedRequestOnOrigin`) (`finished.rs:27-47`).

### 1.2 What git tells it, with no forge

- **Squash-carried:** `worktree::standing::Landed::Carried`
  (`crates/purlis-core/src/worktree/standing.rs:288-300`): every file the branch changed reads
  in the base as the branch has it. **Whole change in history:** `patch_landed`, by
  `git patch-id --verbatim` (`standing.rs:494-525`).
- **Ancestor:** `git merge-base --is-ancestor` of the tip against the base, and against a
  request's head (`crates/purlis-core/src/finished/merged.rs:187-195`).

### 1.3 What it asks the forge, per branch

- `finished::merged::of_the_plane` asks the forge for **the newest request whose head is the
  branch** (`finished/merged.rs:80-113`). The forge's head ref is fetched from
  `refs/pull/<n>/head` or `refs/merge-requests/<n>/head` when the clone lacks it
  (`merged.rs:114-120`, `:174-184`).
- On GitHub that is `GET repos/{o}/{r}/pulls?state=all&head={owner}:{branch}&per_page=1`
  (`crates/purlis-core/src/forge/github/mod.rs:101-146`), **one request per branch**. On GitLab
  it is `GET projects/{id}/merge_requests?source_branch={b}&state=all&per_page=100`
  (`crates/purlis-core/src/forge/gitlab/mod.rs:774-804`).
- **No forge kind but GitHub and GitLab exists** (`crates/purlis-core/src/forge.rs:98-104`).
  Forgejo/Gitea has a reserved slot, FG-13
  (`docs/adr/0070-a-forge-is-one-seam-with-a-native-client-per-forge-and-gh-and-glab-are-its-fallback.md:138-141`).

### 1.4 The polling machinery, built and not yet wired

- **`forge::poll::Poller`** (`crates/purlis-core/src/forge/poll.rs`) holds an account's repeated
  reads and says which are due. It sets the interval by focus (`WORK_VIEW` 60 s, `VISIBLE` 5 min,
  `BACKGROUND` 15 min, none when hidden; `poll.rs:34-39`, `:58-69`). It spreads its reads to at
  most `PACED` = 900 requests an hour (`poll.rs:42`). It backs off 4× below 20 % of the forge's
  limit (`poll.rs:40`, `budget.rs:60`), and it sends nothing itself.
- **Nothing outside the tests calls `Poller::new` or `poll::interval`.** The only use outside
  `poll.rs` is `crates/purlis-core/src/doctor/budget.rs:39`, which reads `BACK_OFF`. The poller is
  proven by `crates/purlis-core/tests/it/a_fifty_request_workspace_polls_inside_its_budget.rs`
  (50 requests, an hour, under 1,000 counted).
- **ETag store:** `crates/purlis-core/src/forge/etag.rs` makes a repeated `GET` conditional.
  It is per account, on the native transport only, in the machine tier
  (`<config>/forge-etags/native-<account>/`, `etag.rs:52-56`). Proven by
  `tests/it/the_native_transport_asks_conditionally_and_once.rs`.
- **Budget meter:** `crates/purlis-core/src/forge/budget.rs:28-36`: 1,000 counted requests an
  hour per account (a fifth of GitHub's 5,000), with background requests held once that is spent.
  A request a person waits on is never held.

### 1.5 The merge queue, CI and the forge proxy

- **There is no merge-queue watcher that polls.** `charter change land` puts a request in the
  queue or train. A queued landing is **logged by a later `land`** that finds the request
  merged at the pending head (`crates/purlis-core/src/change/land.rs:26-38`).
- **CI status** is one GraphQL read of `statusCheckRollup` per ref
  (`forge/github/mod.rs:44-65`), asked when shown, not on a timer.
- **The forge proxy** carries a chat's `gh`/`glab`. It is an allowlist that refuses merges
  unless `guards.merge` lets them through (`crates/purlis-core/src/forge/proxy.rs:14`, `:352`),
  and it refuses other repos (`OtherRepo`, met in this research). **A merge a chat sends
  through it is an act purlis sees**, a named cause under ADR 0076, so the app can make that
  repo's watch due at once.
- **`taskchanges.rs`** (`crates/purlis-core/src/taskchanges.rs`,
  `app/src-tauri/src/taskchanges.rs`) is about which files a task touched. It reads nothing from
  the forge. Its one tie to this question is that it calls `tidy_recorded` when a task ends
  (`app/src-tauri/src/taskchanges.rs:559`).

## 2. GitHub, measured

All on `purlis/purlis`, 2026-11-28, with the person's token through `gh` and the forge proxy.

- **Rate limits of this token** (`GET /rate_limit`): `core` 15,000/h, `graphql` 10,000/h,
  `search` 30/min. A plain user token is documented at 5,000/h *(unverified this session)*.
  purlis's own 1,000/h allowance is fixed whatever the token gets.
- **A 304 is free.** `GET pulls?state=closed&sort=updated&direction=desc&per_page=50` answered
  200 with `ETag: W/"72fe…"`, `X-Ratelimit-Used: 630`. The same request sent twice with
  `If-None-Match` answered `304 Not Modified`, and `X-Ratelimit-Used` stayed at **630** both
  times. The answer also carries `Cache-Control: private, max-age=60`: GitHub does not expect
  the same list to be read more often than once a minute.
- **One page holds two days.** The newest 100 closed requests ran from 2026-11-26T08:42Z to
  2026-11-28T04:11Z, about 2.3 closings an hour. Search counted 58 merged since 2026-11-27. A
  poll every 15 min, or even a day, never falls off the page here. A gap longer than the page
  (the app shut for a week) is caught by the startup backlog read.
- **A list entry has everything the match needs:** `merged_at`, `head.ref`, `head.sha`,
  `head.repo.full_name`, `base.ref`. These are the same fields `request_from` checks today
  (`github/mod.rs:124-146`).
- **GraphQL batches the backlog for almost nothing.** One query with 100 aliased
  `repository.pullRequests(headRefName:"…", first:1, orderBy:{field:CREATED_AT,direction:DESC})`
  answered all 100 branches (69 merged, 15 closed, 16 open) at **`rateLimit.cost` 1**. GraphQL
  has no conditional request, so each send costs at least 1 point. It suits a one-off catch-up,
  not a timer. A query is limited to 500,000 nodes *(unverified)*. 100 × `first:1` is well
  inside that.
- **Search:** `search/issues?q=repo:… is:pr head:A head:B` answered 2, so repeated `head:`
  qualifiers are ORed. An explicit `OR` was refused by the forge proxy. The search limit is
  30/min. A query is documented to hold at most 256 characters and 5 boolean operators
  *(unverified)*. Branch names here run 40 to 50 characters, so about 4 fit in one query, and
  150 branches would take about 38 searches per cycle, more than a minute's limit. Search is
  the wrong tool.
- **The head branch goes when its request merges here:** `delete_branch_on_merge: true`, with
  squash, merge-commit and rebase all allowed. `git ls-remote --heads origin` listed 1,204 heads
  in 1.4 s, at no REST cost. A head missing from origin is a hint, never proof: a person can
  delete a branch by hand, and a request can close with its head kept.

### 2.1 Webhooks

A webhook needs an HTTPS endpoint the forge can reach. A desktop app behind NAT has none. Every
route there needs a public relay: smee.io, which GitHub's docs name for development, or the
`gh webhook forward` extension, which needs repo admin and is meant for testing *(both
unverified this session)*. A GitHub App needs a server of purlis's to receive the events and
pass them on. Each relay is a service purlis would run or depend on, and it would see every
merge of every person's repos. Under the priorities (standard practice, no custom
infrastructure, and the privacy rules in `docs/forges.md` "What purlis does not hold") that cost
outweighs a few minutes of latency. **Not recommended.**

`GET /repos/{o}/{r}/events` and `/notifications` support conditional reads and an
`X-Poll-Interval` *(unverified)*. Events lag by up to minutes, and each page names many kinds of
event, so they are no cheaper than the closed-pulls list, which already has an ETag.

## 3. GitLab and Gitea *(unverified: documentation could not be read here)*

- **GitLab:** `GET /projects/:id/merge_requests?state=merged&order_by=updated_at&sort=desc&per_page=100`,
  with `updated_after=<last seen>`, is the same one-list shape, and `state=closed` gives requests
  closed without merging. Each entry has `source_branch`, `source_project_id`, `sha`,
  `merged_at`. purlis already filters by `source_project_id` (`own_mr`,
  `forge/gitlab/mod.rs:787-789`). GitLab counts every request, 304s included
  (`budget.rs`, the counting rule), so the cost is exactly the cadence: 12 an hour per repo at
  5 min. The documented gitlab.com authenticated limit is far above that (about 2,000 requests a
  minute) *(unverified)*. GitLab's GraphQL has a complexity limit instead of points; it is not
  needed here.
- **Gitea/Forgejo:** purlis has no backend for it yet (FG-13). When it does,
  `GET /repos/{owner}/{repo}/pulls?state=closed&sort=recentupdate` *(unverified)* gives the same
  list. Its rate limiting is set by each instance's admin, and it has no GraphQL. The design
  needs nothing a Gitea lacks.

## 4. What `git fetch` alone tells

- **Free of API points, one round trip per repo** (`git fetch --prune` or
  `git ls-remote --heads`), and forge-agnostic.
- It tells **"landed by content"**: the tip is an ancestor of `origin/main` (a merge commit or a
  fast-forward), or `Carried`/`patch_landed` finds the change in the base (squash, rebase). purlis
  already reads all three (section 1.2). Each is git work per branch, not a request.
- It **cannot** tell:
  - a request that closed without merging;
  - a merge whose head moved after this folder's tip (#5439, why `InMergedRequest` exists);
  - a squash whose files were changed again on main before the fetch (`Carried` then fails;
    `patch_landed` may still hold);
  - whether the missing remote head means merged or deleted.

  So it confirms a merge, and it cannot be the only signal.
- **Its place:** a fetch of the base belongs beside each list read that reports a change, so
  the finished test (which reads the base) sees the merge commit. That is one fetch per repo per
  change, not per branch.

## 5. Options compared, for 150 branches in one repo

| Mechanism | Latency | Requests / hour (5 min cadence) | Counted on GitHub | Tells merged vs closed | Notes |
|---|---|---|---|---|---|
| **Closed-pulls list + ETag (recommended)** | 1 / 5 / 15 min by focus; at once for purlis's own merges | **12 per repo** | only changed answers: ≤ 12, about 3 in practice | yes | needs the `Poller` wired; GitLab counts all 12 |
| Per-branch `pulls?head=` (today's read) | same | 1,800 | up to 1,800, less with 304s | yes | over the 1,000 allowance; the poller would stretch it to 10 min (`150 × 3600 / 900`) |
| GraphQL, 100 aliases a query | same | 24 (2 queries) | 24 points (no 304) | yes | best for the **one-off backlog** at startup, not a timer |
| `search/issues` with ORed `head:` | throttled | about 450 searches | 30/min search limit | yes | about 4 names per query; does not fit |
| Webhook through a relay | seconds | 0 | 0 | yes | needs a public service and repo admin; not standard for a desktop app |
| `git fetch` / `ls-remote` only | cadence of the fetch | 0 API | 0 | **no** | confirms landing by content; cannot see closed-unmerged or a moved head |

## 6. Open points for the spec

- **Wiring.** Where the watches live (one per clone with folders, the plane-wide list per
  account) and what makes one due at once: window shown, Work view opened,
  `purlis worktree push`, a land or a proxied merge. `Poller::due` already applies a changed
  interval at once (`poll.rs:129-131`). An "ask now" needs a small addition.
- **Removal is still the finished test's.** A list match only nominates a folder. A held folder
  or a running task keeps it, as today.
- **Which account.** The ETag store is the native transport's alone, for a human in the window
  (`etag.rs:8-12`). The poll must run as the person in the window, not as a chat. That is also
  what the budget's "background" class expects.
- **A new store?** None is needed. The ETag store and the budget file are already named in
  `docs/plane-format.md` as Machine tier (`plane-format.md:9436-9437`). A remembered "last closed request seen" per repo, if
  added, needs its own tier line (ADR 0069).

## Unverified

These were not re-read this session (docs sites refused), and nothing above depends on them for
the recommendation:

- GitHub: the 5,000/h user-token primary limit; the secondary limits (points per minute,
  concurrency); search's 256-character, 5-operator and 1,000-result caps; GraphQL's
  500,000-node limit; that events and notifications honour `X-Poll-Interval`; smee.io and
  `gh webhook forward` as documented relays. Pages:
  docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api,
  …/rest/using-the-rest-api/best-practices-for-using-the-rest-api,
  …/rest/search/search, …/graphql/overview/rate-limits-and-query-limits-for-the-graphql-api,
  …/webhooks/testing-and-troubleshooting-webhooks/testing-webhooks, …/rest/activity/events.
- GitLab: the `merge_requests` list parameters (`updated_after`, `order_by`), ETag support on that
  list, the gitlab.com rate limits. Pages: docs.gitlab.com/api/merge_requests/,
  docs.gitlab.com/user/gitlab_com/#rate-limits-on-gitlabcom.
- Gitea/Forgejo: the `pulls` list's `sort=recentupdate`, rate limits set by the admin. Page: the
  instance's `/api/swagger`.
