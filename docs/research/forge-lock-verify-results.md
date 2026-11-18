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
