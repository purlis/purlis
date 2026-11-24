---
name: landing-a-pr
title: Landing a pull request
description: Open a lane's purlis pull request as a draft, mark it ready once, and land it on main through the merge queue. Use when opening a PR, when a lane reports done, when a PR is green, when someone asks to merge or land one, or when the merge queue dropped one.
---

`node tools/land.mjs <pr> --sha <reviewed sha> --watch` does every mechanical step, refuses whatever is not ready, and says why. Its header comment lists what it checks. This skill is the judgement around it. Each step ends on its **done** line.

## Draft, then ready once

A lane opens its pull request as a **draft**. A draft runs only the fast guards and web tests (the ready gate, [ADR 0127](../../../adr/0127-a-pr-run-is-a-subset-of-the-queue-run-gated-by-ready.md)), so iterating on it is quick. When the lane's local checks pass and it reports done, it marks the pull request ready **once** (`gh pr ready <pr>`), the one time the heavy jobs whose scope fires start. Never flip it back and forth: each ready starts them again.

- **Marking ready starts more checks.** Wait for them to finish, green, before enqueueing (step 3). A `gh pr ready` is not the end of the checks.
- **A chat cannot run `gh pr create`,** so the line it hands the operator is `gh pr create --draft …` while the lane has not reported done, and the same line without `--draft` when it has: a pull request opened after the report is born ready.

**Done:** the pull request was a draft until the lane reported done, was marked ready once, and its heavy checks are green at the head you will land.

## 1. Reviewed

A reviewer agent other than the author approved the pull request at its current head SHA. A change to consent, permissions, a sandbox or secrets gets an **adversarial** reviewer, one told to break it. The author hands its work to a reviewer and lands it once the review approves.

A small fix after approval lands on that approval when the new commits only answer the review's findings and add no behaviour. Anything more goes back for another round.

**Done:** you can name the reviewer's approval and the SHA it approved, and that SHA is what you pass as `--sha`.

## 2. Shaped

- Tickets that touch the same area or the same files travel together: one pull request, one commit per ticket, a `Closes #n` for each, so they share one CI run and one landing on a moving main. A risky part (consent, security, a public key), or one that would hold the rest back, gets a pull request of its own.
- Every design choice the author settled is a numbered D-line (`D-<ticket>-n`) in the body.
- Every leftover is a ticket in the milestone, linked from the body.

**Done:** the body closes every ticket it delivers, lists its D-lines, and links a ticket for each leftover.

## 3. Landed

Run `tools/land.mjs` with `--watch` and act on what it says. A pull request still in draft is not landed: mark it ready first, and wait for the checks that starts.

**Runs held for approval are the operator's.** When a run of the head waits on approval, the tool names each one (name, id and link), says the operator approves them, and exits with status 4 without reading a check or enqueueing anything. Never rerun or approve such a run yourself, and never start a fresh run some other way (an empty push, `gh pr update-branch`) to get past it. Hand the run ids to the operator, and land again once they have run (#3991).

When it reports a drop from the queue, read the annotations it prints:

- **The pull request's own failure:** fix it, review the fix (step 1), land again.
- **A merge conflict:** it prints no annotations, since the queue built nothing. Bring main into the branch, resolve the conflict, review the new head (step 1), and land again.
- **A known flake:** find or file its ticket (label `flaky`), link it in a comment on the pull request, and land once more. A second drop on the same check is a real failure.
- **A `Flaky (passed on retry N)` warning on a green run:** nextest retried a Rust test once and it passed (`.config/nextest.toml`). Nothing blocks the landing, but find or file the test's `flaky` ticket and link it, as for a red one: the retry saves the rerun, it does not fix the flake.
- **Main itself is red:** landing stops for everyone. Open or join the pull request that fixes main, labelled `fixes-main`, and land that first; then land this one again.

**Done:** `tools/land.mjs` printed that the pull request merged.

## 4. Settled

Watch main's `ci` and `release` runs for the merge commit through to green; a red one is now step 3's "main itself is red". Then delete the clone's build output (`target/`, `node_modules`) and its worktree.

**Done:** main's `ci` and `release` are green on the merge commit, and the clone is cleaned up.
