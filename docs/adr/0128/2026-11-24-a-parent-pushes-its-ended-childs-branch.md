# Amendment 2026-11-24: a parent pushes its ended child's branch, and only that

Amends [ADR 0128](../0128-your-sign-in-brokered-one-approach-for-every-forge.md), D8 (what the
route lets a chat write), narrowing the dispatcher follow-up of
`2026-11-23-a-refused-push-says-why-and-the-asking-chat-follows-up-its-tasks-branch.md` and keeping
the person's Allow of `2026-11-23-the-person-may-let-a-chat-push-a-branch-it-has-no-record-of.md`
as it was. The person's ruling of 2026-11-24 (all six points accepted), built after #7530, which
keys the record by a project id every plane has.

## What was wrong

A chat could update the branch of **any task it dispatched, directly or eight links up the chain,
running or ended, of any persona and workspace, on any branch the task pushed**. That is wider than
the reason for it (an ended task's branch is its asker's to finish) and it had no switch.

## Decision (the ruling's six points)

1. **Lineage is a direct child, in the same workspace, run by the same persona, whose task has
   ended, on the branch purlis cut for that task.** Each is read from the app's own dispatch
   record (`dispatchrecord::ended_child_branch`): a `task` record whose asker is the pusher and
   whose worker is the owner, by ULID (no grandchild, no handoff); `ended` is set and no record of
   that worker still runs; `asker.workspace` is `place.workspace`; the asker's persona is the
   persona dispatched to, both named; and the branch is exactly `place.worktree.branch`. A running
   task, a grandchild, another persona, another workspace, a branch the task made that purlis did
   not cut for it, and any record that does not read as the app's own, stay what they were: the
   route refuses, and where the repo's record allows it, the person is asked (*Let this chat push
   it*).
   - **The owner line must be a route push.** The created-branch record gains one optional field,
     `via`: `"route"` for a push the forge confirmed through the route, `"allow"` for the person's
     Allow (`docs/plane-format.md`, first). Lineage reads only `"route"`; a line without `via`
     (written before the key) is neither, and is never lineage. The claim log and any file a chat
     writes are never read for it. The same marker is what a later change that must tell the
     person's Allow from a push needs.
2. **Lineage is a policy row, on by default.** It is the rule `forge.lineage` of the
   `guards.forge` family (`docs/policy-inventory-families.md`): `[policy]`'s `guards.forge` set to
   `off` is no lineage, and `log` and `enforce` leave it on. It is an **allowance**, so unlike the
   family's refusals it has nothing to log under `log`, and it is on under every template but
   `open`. It follows the family's rules: only the app resolves it, a loosening (here, switching
   lineage off is a tightening of this row but a loosening of the family) waits for the person's
   press, and an administrator's lock keeps the family, so lineage, on.
3. **Integration branches are protected by a committed name pattern, first.** The route's
   `AllowedRepo` had `with_protected_branches` and no caller. It now has one (`forgeroute`'s
   `repos`), fed from `[risk] protected_branches` in the plane's committed `charter.toml`, the key
   the ask's risk class already reads (`askrisk::committed_protected_branches`; names, or patterns
   with `*`). The route also protects, with no setting, `phase-*`, `integration/*`,
   `integration-*` and a milestone's `m<N>-*` branch (`m75-policy`, `phase-1-proven`), beside the
   default branch and the release and deploy names it always protected. **Lineage never reaches a
   protected branch**: the route refuses one before it reads an owner, so only the merge Ask lands
   on it.
4. **No cross-workspace master.** There is no chat that may follow up every workspace's branches;
   the workspace is part of the lineage test, and nothing else widens it.
5. *(Left to a later ruling; nothing in this change.)*
6. **Lineage never reaches a delete, and the person's Allow never chains to an ancestor.** A delete
   of any branch is refused before an owner is read, as it was. The Allow line is `"via": "allow"`
   and so is not lineage: a chat the person let push a branch is its owner, and the chat that
   asked for it gains nothing from that press.

The confirmed push is recorded under the pusher as a route push, so the task can no longer move the
branch and the parent goes on from the new tip. The link still runs one way.

## Tier

No store is added. `docs/plane-format.md`'s row for `forge-created-branches.jsonl` (Machine,
device-bound) names the one optional field `via` and the narrower follow-up. The setting is the
committed `[risk] protected_branches` key, which already exists.
