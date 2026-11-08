---
name: purlis-change
description: When work in a purlis workspace spans several repos and should land as one cross-repo change, a branch and pull request per repo. Use when asked to start or track a cross-repo change, or land its pull requests in order (`purlis change`). Not for one repo's branch (purlis-repo-work).
---

# One change across several repos

A **change** is one piece of work across several repos, kept as a record of intent in the
workspace (ADR 0060). Each repo in it is a **member**, with its own branch and its own pull or
merge request (a **request**). The record holds why, the members, their branches and which
member must land before which (`needs`). Nothing the forge or git knows is stored in it: which
requests exist, their checks and what landed are read again each time.

Work in one repo, however many branches, is not a change: that is purlis-repo-work.

## 1. Read where it stands

| To know | Use |
|---|---|
| The workspace's changes, or one change's record (why, members, branches, needs) | the `change_status` tool, or `purlis change list` |
| Each member's request, its checks at the request's head, what landed and what waits | `purlis change show <slug>` |

`change_status` reads the record alone and never asks the forge. `purlis change show` asks the
forge as the person, so read it before any push or land. Checks are one of `PASSED`, `FAILED`,
`RUNNING`, `NOT RUN` and `UNKNOWN`, read at the request's head commit only: a commit pushed
after the checks ran sets the member back to `NOT RUN`. purlis never waits for checks; read
`show` again later.

## 2. Create it and name its members

```bash
purlis change create <slug> --why "<one line: what this work is for>"
purlis change add <slug> <repo>                          # branch change/<slug>
purlis change add <slug> <repo> --branch <branch> --needs <other-repo>
purlis change drop <slug> <repo> --why "<why this repo is out>"
```

- One repo per `add`, by its exact name. There is no pattern and no `--all`.
- A member is a repo cloned in this workspace. Where it is not, `add` refuses and names the
  fix, `purlis clone <repo>`.
- `--needs <repo>` (repeatable) names a member that must land **before** this one: the API
  before its consumers. Declare it when adding the member. A cycle is refused with both
  members named.
- The slug is the default branch name, `change/<slug>`, and is on every request and landing
  commit, so pick one that says the work.

## 3. Do the work in each member

Each member's work happens in its own repo, on the member's branch, the way purlis-repo-work
says: its own build and tests, a branch folder per branch, `purlis worktree commit` to commit.
`purlis change push` commits nothing, so every member's work is committed first.

## 4. Push every member

```bash
purlis change push <slug>
```

It prints every repo, branch and destination, then pushes each member's branch, opens or
updates its request (titled `<slug>: <repo>`, with the change's why) and writes into every
request a block linking all the others. It never forces: a push only creates a branch or moves
it forward. A member that cannot be pushed is named and the others still go (exit 1). A push
publishes the person's work, so push when the person has asked for it. The window's changes
view (**Open changes** in the command palette) has the same Push, and shows what it will do
before it does it.

## 5. Land in order, one member at a time

```bash
purlis change land <slug> --repo <repo>             # a merge commit
purlis change land <slug> --repo <repo> --squash    # one squashed commit
```

`land` merges a member's request, and only when every gate holds. Each refusal is named:

1. **One member per run.** `--repo` is given once; there is no way to land them all.
2. **What it needs has landed first.** A member lands only after every member in its `needs`
   has landed **through purlis**: the forge reports the request merged and the default branch
   holds the commit purlis's landing log recorded. So land the members that need nothing
   first, then those waiting on them.
3. **Its checks `PASSED` at the request's head.** Not at an earlier commit; a head that moved
   since the checks were read is refused.
4. **Not by rebase.** `--rebase` is refused: it leaves no single commit to carry the change's
   name or to revert. `--squash` is the person's choice, except on a GitHub merge queue, whose
   own rule sets the method.

Where the target branch has a merge queue (GitHub) or a merge train (GitLab), `land` puts the
member in it, and the queue merges it later. Run `purlis change land <slug> --repo <repo>`
for that member again once it has merged: that run records the landing, and its dependents
can land only after it.

A request merged in the browser, by anyone, is never recorded as purlis's landing, and a
member waiting on it is refused. Tell the person this before they merge a member by hand.

**Landing merges code, so it is the person's act.** Run `land` only on the person's word in
this chat, member by member, or leave it to the Land button in the changes view, which asks
them first. purlis refuses `land` outright in a run nobody is watching.

## 6. Roll it back

```bash
purlis change revert <slug>
```

It seeds a new change, `revert-<slug>`: in each member purlis landed, a branch
`change/revert-<slug>` off the default branch with `git revert` of the landed commit, and the
order turned round (a member reverts before what it needed). It pushes and merges nothing:
push and land `revert-<slug>` as in §4 and §5. A member merged outside purlis has no landing
record and is named as the person's to revert by hand. After fixing a refusal, run it again:
it seeds only the members not seeded yet.

## 7. End it

```bash
purlis change forget <slug>
```

It deletes the record only. Branches, requests and the landing log stay.
