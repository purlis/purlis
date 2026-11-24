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
   whose worker is the owner, by ULID (no grandchild, no handoff); `ended` is set, and **no record
   naming that worker, of any mode (a running handoff too), has not ended, none that does not
   read as the app's own, and no record file torn or unreadable that names it** (any of them means
   the child still runs); `asker.workspace` and `place.workspace` are both named and the same; the
   asker's persona is the persona dispatched to, both named; and the branch is exactly
   `place.worktree.branch`. A running task, a grandchild, another persona, another workspace, a
   branch the task made that purlis did not cut for it, and any record that does not read as the
   app's own, stay what they were: the route refuses, and where the repo's record allows it, the
   person is asked (*Let this chat push it*).
   - **The owner line must be a route push, and the Allow stays one.** The created-branch record
     gains one optional field, `via`: `"route"` for a push the forge confirmed through the route,
     `"allow"` for the person's Allow (`docs/plane-format.md`, first). Lineage reads only
     `"route"`; a line without `via` (written before the key) is neither, and is never lineage.
     **A later push of the same chat keeps the earlier marker** when that was the Allow or none:
     the child's next route-confirmed push on a branch the person let it push is written
     `"allow"` again, so the Allow does not become a route line one push later and never chains
     to the parent. The claim log and any file a chat writes are never read for it.
2. **Lineage is an allowance of its own, on by default, whose strict value is "denied".** It is
   `[policy] lineage = "allow" | "deny"`, **not a row of `guards.forge`** (round 2 of #7552: that
   family's `off` is the loose end, so hanging lineage on it turned lineage on for a tightening
   of `off` to `log`, for an administrator's lock, and for every path that fails closed). It is
   read as `[dispatch.denies]` is (`enforcement::lineage_in_force`): from the committed file at
   the plane root's `HEAD`, never fewer denials than this person accepted (kept under `lineage`
   in `policy_accepted`).
   - **Nothing set means allowed**, as ruled. `deny` in the committed file is a tightening and
     holds at once; taking it away, or `allow`, is a loosening that waits for the person's press
     (*lineage through the forge route, from denied to allowed*). The local file may add `deny`,
     never remove it.
   - **An administrator's lock holds it off**: `sandbox.lineage: false` in
     `/etc/purlis/policy.json`, written as `run-grants` is. A policy file refused closed holds
     it off too.
   - **Anything purlis cannot read means no lineage**: a value that is neither word, a file that
     is not TOML, a `[policy]` that is not a table, any other value in the file that did not
     read, a `HEAD` git could not read (never kept as accepted), and a broken acceptance.
   - It has no `guards.forge` coupling and no rule id in the enforcement log: it is an allowance
     with nothing to log. Only the app's route asks.
3. **Integration branches are protected by exact names the project commits.** The route's
   `AllowedRepo` had `with_protected_branches` and no caller. It now has one (`forgeroute`'s
   `repos`), fed from `[risk] protected_branches` in the plane's committed settings file, the key
   the ask's risk class already reads (names, or patterns with `*`). **The route builds in no
   prefix shape**: `m<N>-*` and `phase-*` were removed in round 2, because they refused ordinary
   task branches (`m75-20-grants-and-nevers-7029-…`, `phase-1-proven-prs-take-the-new-base-…`) for
   every chat push, before the ask. The route protects the default branch, the release and deploy
   names it always protected, the names in the list and the one explicit namespace
   `integration/*`. A project that wants `m75-policy` or `phase-1-proven` protected lists them;
   **the plane's committed settings file belongs to the plane's repository, not purlis's**, so the
   person adds `[risk]` / `protected_branches = ["m75-policy", "phase-1-proven"]` there.
   - **The list is read as `[policy]` is** (`enforcement::protected_branches_in_force`): from
     `HEAD`, never the working tree (a chat could drop a protection by editing the file without
     committing), and never fewer than the last accepted copy (kept under
     `risk.protected_branches`), so a file that does not parse loses nothing. A name a commit
     adds protects at once; one it drops waits for the press.
   - **Lineage never reaches a protected branch**: the route refuses one before it reads an
     owner, so only the merge Ask lands on it.
4. **No cross-workspace master.** There is no chat that may follow up every workspace's branches;
   the workspace is part of the lineage test, and nothing else widens it.
5. *(Left to a later ruling; nothing in this change.)*
6. **Lineage never reaches a delete, and the person's Allow never chains to an ancestor.** A delete
   of any branch is refused before an owner is read, as it was. The Allow line is `"via": "allow"`
   and so is not lineage, and stays so through the owner's later pushes (point 1): a chat the
   person let push a branch is its owner, and the chat that asked for it gains nothing from that
   press.

The confirmed push is recorded under the pusher as a route push, so the task can no longer move the
branch and the parent goes on from the new tip. The link still runs one way.

## What lineage is not

- **"Ended" is sequencing, not protection from the parent.** It says the child no longer runs, so
  the two never push the branch at once. It does not shield the child's work from the parent.
- **The parent may rewrite (force-push) an ended child's branch**, from the child's last tip: the
  same power as an owner, which lineage hands it. A person who wants a branch the parent cannot
  move lists it in `[risk] protected_branches`, or denies lineage.

## Tier

No store is added. `docs/plane-format.md`'s row for `forge-created-branches.jsonl` (Machine,
device-bound) names the one optional field `via` and the narrower follow-up. The settings are the
committed `[policy] lineage` key (new) and the `[risk] protected_branches` key (existing), both of
the Plane tier; their accepted copies are two more pairs of `policy_accepted` (Clone state,
already named), and the lock is `sandbox.lineage` of the machine policy file (Machine, already
named).
