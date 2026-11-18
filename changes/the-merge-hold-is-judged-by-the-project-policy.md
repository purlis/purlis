### Changed

- **A chat's merge is judged by the project's committed policy.** The session host decides it
  from the policy at the plane root's `HEAD`. Under `strict` it is the same merge ask as before,
  and the refusal now names the rule that holds it, `guards-merge-hold`, with its level and
  owner. A rule that denies merges refuses them by name and asks nobody. Where the session host
  can't be asked, the merge is refused and the chat is told so. The event log's blocks, holds and
  raised asks carry the deciding rules (M75 slice 04, #7012).
