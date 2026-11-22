# Amendment, 2026-11-22: the route carries a chat's GitLab CI and land reads (#7372)

[The amendment of the same day](2026-11-22-the-route-carries-a-chats-ci-and-land-reads.md) taught
the route to carry a chat's CI and landing work on GitHub. This one does the same for **GitLab,
gitlab.com and self-hosted, for the chat's recorded project only**, and keeps the same lines: what
the amendment above refuses stays refused, and merges are the merge guard's.

A GitLab REST path names its project by URL-encoded path or by numeric id, and both are resolved
and compared to the recorded project before any route below is looked at: a path that is another
project (`group%2Fother`, `proj.git`), an id that is not the one the person's confirmation recorded,
and **any id for a project with no recorded id** are `OtherRepo`. Nothing here changes that.

## What is added

1. **Rules are a read.** `GET protected_branches`, `protected_branches/{name}` and
   `approval_rules`, and `merge_requests/{iid}/approval_state`. These are what `tools/land.mjs`
   reads to know what a merge request needs. Every other route under a rule (`approval_settings`,
   `push_rules`, `protected_tags`, a rule's own sub-routes) and **every write**, to a protected
   branch or an approval rule, is still `Ruleset`.
2. **A merge train is a read.** `GET merge_trains` and `merge_trains/merge_requests/{iid}`. Joining
   a train (`POST merge_trains/merge_requests/{iid}`) is a merge and stays off the list, with
   `merge_when_pipeline_succeeds`; the merge guard owns it.
3. **Pipelines and jobs are read, and a job's log and artifacts are downloaded.** `pipelines`,
   `pipelines/latest`, `pipelines/{id}`, `pipelines/{id}/jobs`, `jobs/{id}`, `jobs/{id}/trace`,
   `jobs/{id}/artifacts` and `jobs/artifacts/{ref}/download?job=` are answered (most already were).
   `pipelines/{id}/bridges`, which names the projects a pipeline triggers, and `variables` are not.
4. **A pipeline or a job is retried or cancelled.** `POST pipelines/{id}/retry`, `pipelines/{id}/cancel`,
   `jobs/{id}/retry` and `jobs/{id}/cancel`, on the recorded project, with no query and a body that
   is empty or `{}`. They are `Class::CiWrite`, the class the GitHub rerun has, so the audit says
   what was done. The reasoning of the GitHub amendment holds as it is written there: a retry
   replays a job **with its secrets**, so a chat can retry an old deploy job on the default branch
   and redeploy a stale commit; a protected environment's approvals still hold, since the retried
   job waits on them as the first did. The person who does not want that signs the chat out of the
   forge.

## What is held for the person

**`jobs/{id}/play` is refused, as `Refusal::Approval`.** It starts a manual job, which is a gate a
pipeline's author put in front of code that deploys, releases or runs on a protected ref, and it
takes `job_variables_attributes`, so the chat would also choose the job's environment. That is the
reason GitHub's `approve` is refused: it runs gated code. It is the person's to press, and a
hand-off for it is a new kind of ask and an executor in the app, which is more than this amendment.
`play` is on the approval names, so no body changes the answer. **The trigger routes are not added**
(`trigger/pipeline`, `ref/{ref}/trigger/pipeline`, `POST pipeline`, `POST pipelines`): each starts
a pipeline with variables of the caller's choosing. A pipeline's own retry is not that: it replays
the variables the pipeline already had.

A retry is not a way round it either, as far as the route can tell: it starts nothing that has not
run, since a retry of a pipeline runs its failed jobs only. Whether GitLab also lets `jobs/{id}/retry`
start a job that is still `manual` was not checked; if it does, `jobs/{id}/retry` is narrowed here
by an amendment.

## A job's artifacts

gitlab.com answers `jobs/{id}/artifacts` and `jobs/artifacts/{ref}/download` with a `302` to Google
Cloud Storage, to a signed address. A self-hosted GitLab serves them from its own host, with no
redirect, and `jobs/{id}/trace` is answered by the API itself. `glab` reaches the listener inside
TLS, where it has no way to follow a redirect to a host the chat's egress refuses, so the listener
follows it, as it follows GitHub's CI storage, with the same limits:

- **Once, by a GET, with no credential**, and only the headers `accept`, `range`, `if-none-match`
  and `if-modified-since` are sent on. Never after a write, never a second redirect, never for a
  public read (no credential was sent, so no address was signed for it).
- **Only for gitlab.com's chat, and only to `storage.googleapis.com` under the path
  `/gitlab-gprd-artifacts/`.** A bare `storage.googleapis.com` is anyone's bucket, as
  `*.blob.core.windows.net` is, so the bucket's name is part of the match, with the address as the
  URL parser left it (a `..` is resolved first), the `https` scheme, port 443 and no login in it.
  A bucket as a subdomain, a name that merely starts like it and a host spelled like it are not it.
- **A self-hosted GitLab's redirect is not followed**, whatever it points at: its object storage is
  its operator's choice of host, and the follow is the one place the listener fetches an address
  the forge named. The chat is answered `502`, as it was. The rule is by forge: a GitHub chat is
  not given GitLab's bucket, nor a GitLab chat GitHub's hosts.
- The address is never written for the chat to tunnel to, and the hosts are on no tunnel list.

**What this rests on, and what was not checked.** The bucket name is what gitlab.com's artifacts
redirect pointed at when this was written, taken from the task's description of it. It was not
read from a live gitlab.com response, which the sandbox this was made in could not reach. If
gitlab.com serves artifacts from another host or bucket (a CDN in front of the bucket, say), the
chat gets `502` for it, which is the safe way to be wrong, and the host is added here by an
amendment once it is read, not before.

## What is not changed

- Merges and the merge routes, reviews and approvals that count toward a merge, releases and tags,
  repository administration (`variables`, `runners`, `hooks`, `settings`…), and every write to a
  rule stay refused, each with the refusal it had.
- `Forwarded` now says which forge decided the request (`kind`), which the redirect rule reads;
  nothing else is added to what the listener sends the chat.
- `glab ci view` draws a full-screen view and asks for a terminal, so the integration test runs
  `glab ci list`, `ci trace` and `ci retry` against the listener. Whether `ci view` also reads
  `pipelines/{id}/bridges` is not known here, and that route stays off (above): if the real `glab`
  needs it, it is a follow-up that reads what the answer names before it is added.
