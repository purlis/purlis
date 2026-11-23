# tools/r7

Measurement kit for R7 (what thin LTO costs a dev build, spec #7324). On the branch
`release-r7-measure-*` only, never merged. Adapted from R0's `tools/r0` (branch
`release-r0-measure-one-dev-build-rf54khyy`) to R1's one compile. `.github/workflows/release.yml` on
that branch is the measurement workflow: `workflow_dispatch`, no secrets, publishes nothing.

- `step.sh` times a named step into `steps.tsv`.
- `sample.sh` samples memory and swap every 2 s.
- `cc-shim.py` and `dsymutil-shim.py` time every link and every dsymutil run.
- `ts.py` timestamps a log.
- `summarize.py` turns one job's records into `summary.md` and `summary.json`.
