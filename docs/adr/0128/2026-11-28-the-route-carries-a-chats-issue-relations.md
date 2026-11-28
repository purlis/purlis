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
- **The body is exactly the row's keys**, a JSON object, each id a positive JSON integer, the
  type GitHub's schema gives it (`gh api -F`, never `-f`); anything else is a malformed request. `replace_parent` is refused whatever it says: it
  takes the sub-issue from its parent, which can be another repo's issue.
- **A refusal no setting can lift names who can**: "No setting can allow it: only purlis's
  maintainers can, by changing purlis." A floor act already names the person, on the forge.

## What it accepts

- **An id's place is the run's cached answer**, the same one every GraphQL write tied to an id
  reads (D3e). An issue the person moves to another repo of the same owner during the run can
  still be linked from the chat's issue until the run ends. Moving an issue is the person's act,
  and the link it allows is to an issue of the same owner, which GitHub requires anyway.
- **An id the forge could not place is refused as another repo's**, as an unplaced GraphQL id
  is; the chat may try again.

## Not decided here

- **The relations' reads stay refused** (`GET issues/{n}/sub_issues` and the dependency lists):
  they return other repos' titles. A chat reads a map's children through `gh issue view`'s
  GraphQL, which D3e answers for the repo, until a repo-scoped read is decided.
- GitLab's relations (its links and work-item hierarchy) and GitHub's GraphQL relation mutations
  (`addSubIssue`, `addBlockedBy` and theirs) stay refused until a chat needs them.
