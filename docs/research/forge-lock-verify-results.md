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
