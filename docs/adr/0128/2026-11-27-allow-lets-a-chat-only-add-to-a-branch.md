# Amendment 2026-11-27: Allow lets a chat only add to a branch

Amends [ADR 0128](../0128-your-sign-in-brokered-one-approach-for-every-forge.md), D8, and
`2026-11-23-the-person-may-let-a-chat-push-a-branch-it-has-no-record-of.md`, which said that after
Allow "the chat can replace the branch's history, not only add to it". Floor item 6 of
[ADR 0125's 2026-11-27 amendment](../0125/2026-11-27-what-the-policy-rules-between-chats.md) rules
that there is no force push onto another chat's branch, in any mode (#7841). The steward's call of
2026-11-27, for the person's review.

## The decision

- **Allow still records the chat as the branch's owner** at the tip the forge holds at the press.
- **A branch the chat owns by Allow moves only forward.** The record's line keeps its `allow`
  marker through the chat's later pushes (`record_via`), and every update of such a branch is
  forwarded only where its new tip descends from the branch's tip on the forge. Anything else is
  refused with no ask: no press, mode or level reaches the floor.
- **The ask no longer says Allow can replace the branch's history.** It says the chat can add to
  it.
- **A pull request's head the chat owns by Allow is not written to by update-branch or a GitLab
  rebase**: the forge would write it, and a rebase replaces its history. The chat merges or
  rebases locally and pushes, which is checked.

## How it is checked

The forge route sees a push's old and new ids, never its history, so the policy marks the update
as fast-forward only and the push gate, which already reads every push in a quarantine before it
is sent (D8.3), decides it: the quarantine holds the branch's tip, fetched from the forge as the
person at that push, and the push's own pack, and `git merge-base --is-ancestor` answers. The
call named the forge's compare API (`GET compare/{base}...{head}`, GitLab's
`repository/compare`); it cannot answer here, because the new tip is not on the forge until the
push is sent, and a compare of a commit the forge does not hold is not found. The quarantine's
answer is the same question asked of the same objects, with the forge's tip read as the person.

## Not changed

A parent taking up its ended child's cut branch (the 2026-11-24 amendment) is a route push, never
an Allow, and is not touched here.
