# Forge credential lock: slice 0 live results (#7133)

Recorded by `tools/forge-lock-verify/person.sh` and `chat.sh` (design: `forge-credential-lock.md`
section 11). One section per run, appended by the scripts, never edited by hand. Result is `pass`
(the observed answer is the expected one), `fail` (it is not: for a row expected refused, the
lock has a gap), `inconclusive` (refused for a reason that proves nothing) or `info` (an
observation, no verdict). Rows R2 to R4 are expected refused because that is the lock's aim; the
design (6.2) predicts them allowed, so a `fail` there confirms the named gap.

Not covered by the kit, to be done by hand or dropped: S1 (the app's own OAuth login creating a
ruleset), Q1 (merge queue: not offered on a personal repo), GitLab.

Account plan, date and API version header of the run: _to fill by the person after the run_.

## 2026-11-18T16:28Z · chat.sh · A · as chat · purlis/purlis-forge-lock-verify-1

| row | expected | observed | result |
|---|---|---|---|
| M0 | allowed 200 (no ruleset: the test can pass) | HTTP 200 Pull Request successfully merged  | pass |
| P5-ctl | allowed 201 (control: create refs/heads/release/ctl) | HTTP 422 Reference update failed  | fail |

Note on P5-ctl: the 422 is not the lock. A branch named `release` exists, and git cannot hold `release` and `release/ctl` together (a directory/file ref conflict), so no actor can create `refs/heads/release/*` while `release` exists. The same holds for P5 and P6 (`release/new`, `release/x`) in phase B: their refusals would prove nothing. The kit's P5 and P6 need a different protected namespace (or the control a different ref) before B runs.

## 2026-11-18T16:33Z · person.sh · lock · as diazoxide · purlis/purlis-forge-lock-verify-1

| row | expected | observed | result |
|---|---|---|---|
| B3 | accepted or refused (decides person vs role) | HTTP 201  : actor_type User accepted | info |
| RS1 | branches ruleset created (bypass User, mode pull_request) | HTTP 201   | pass |
| RS2 | tags ruleset created (bypass User, mode always) | HTTP 201   | pass |

## 2026-11-18T16:48Z · person.sh · press · as diazoxide · purlis/purlis-forge-lock-verify-1

| row | expected | observed | result |
|---|---|---|---|
| B2a | refused (your direct write to main, bypass mode pull_request) | HTTP 409 Repository rule violations found  Cannot update this protected ref.    | pass |
| B2b | allowed 200 (the person's press still merges) | HTTP 200 Pull Request successfully merged  | pass |
| T-admin1 | allowed 201 (your tag, bypass always) | HTTP 201   | pass |
| T-admin2 | allowed 204 (delete your tag) | HTTP 204  | pass |
| W1c | run of the chat's edited script fails (its GITHUB_TOKEN could not write main); success is a CI-route gap | latest run: completed / failure | info |
| W1d | ci-wrote.txt absent from main | HTTP 404 Not Found  | pass |

## 2026-11-18T23:40Z · person (gh pr merge --auto, then steward read) · B3-person · as diazoxide · purlis/purlis-forge-lock-verify-1

| row | expected | observed | result |
|---|---|---|---|
| B3-person | a person-enabled auto-merge must not merge a chat's pushed commit through the person's bypass | PR #7 head 46afe13 ('push after auto-merge', author purlis-purlis-agent[bot]); auto-merge disabled, then enabled by diazoxide (--auto --merge); ci green; after more than 5 min: merged=false, mergeable_state=blocked, main unmoved at 20ed14a8b | pass |

Reading: under the branch ruleset's `pull_request` bypass mode, GitHub's auto-merge does not use the enabler's bypass, so a chat's commit cannot land on a protected branch through an auto-merge the person turned on. The design's "warn when Allow auto-merge is on" stays as defence in depth, not as the only guard.
