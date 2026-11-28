# Amendment 2026-11-28: the route carries a chat's issue relations on its own repo

Amends [ADR 0128](../0128-your-sign-in-brokered-one-approach-for-every-forge.md), D3d (the REST
allowlist), and §10 of the amendment of 2026-11-27, "forge operations under policy" (refusal
text). The steward's call of 2026-11-28 on #8022, for the person's review.

## What was wrong

A chat charting a map on its own repo was refused, with "no setting can allow it", for the
sub-issue and blocking-link writes `docs/agents/issue-tracker.md` names, while the route carried
the same issue's create, comment and update. The refusal named nobody who could change it.

## The decision

- **GitHub's issue relations are carried on the chat's own repo, as the issue's update**
  (`IssueWrite`, the class the issue's `PATCH` has; M75's typed operations take them over):
  `POST issues/{n}/sub_issues` (`sub_issue_id`), `DELETE issues/{n}/sub_issue` (`sub_issue_id`),
  `PATCH issues/{n}/sub_issues/priority` (`sub_issue_id`, and `after_id` or `before_id`),
  `POST issues/{n}/dependencies/blocked_by` (`issue_id`) and
  `DELETE issues/{n}/dependencies/blocked_by/{issue_id}` (no body). On another repo each stays
  refused, as every write there is. The reads of these routes stay refused (D3d): they return
  other repos' titles.
- **Every issue a relation names is placed in the chat's repo before the write goes.** The body
  names issues by GitHub's numeric id, which can be an issue of any repo of the same owner. The
  route asks the forge, through the resolver that places GraphQL ids (D3e), which repo the node
  `05:Issue<id>` (base64) is in: GitHub's first form of a node id, which it still reads, with a
  deprecation warning, as the issue of that id whatever its repo. An id in another repo, or one
  the forge can't place (it no longer reads that form, it could not be reached), refuses the
  write as another repo's.
- **The body is exactly the row's keys**, a JSON object, each id a positive integer or its
  digits; anything else is a malformed request. `replace_parent` is refused whatever it says: it
  takes the sub-issue from its parent, which can be another repo's issue.
- **A refusal no setting can lift names who can**: "No setting can allow it: only purlis's
  maintainers can, by changing purlis." A floor act already names the person, on the forge.

## Not decided here

GitLab's relations (its links and work-item hierarchy) and GitHub's GraphQL relation mutations
(`addSubIssue`, `addBlockedBy` and theirs) stay refused until a chat needs them.
