---
name: landing-a-pr
title: Landing a pull request
description: Land a purlis pull request on main through the merge queue. Use when a PR is green, when someone asks to merge or land one, or when the merge queue dropped one.
---

`node tools/land.mjs <pr> --sha <reviewed sha> --watch` does every mechanical step, refuses whatever is not ready, and says why. Its header comment lists what it checks. This skill is the judgement around it. Each step ends on its **done** line.

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

Run `tools/land.mjs` with `--watch` and act on what it says.

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
