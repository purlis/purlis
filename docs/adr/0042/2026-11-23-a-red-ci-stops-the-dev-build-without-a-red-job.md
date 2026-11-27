# Amendment, 2026-11-23: a red ci stops the dev build without a red job

The wait before signing (see the amendment of the same day, compiled before the gate) failed the
`build` and `runner-binary-aarch64` jobs when the commit's `ci` ended red. Nothing was signed or
published, which was right, but the dev build showed as a failure and alarmed people for a result
that was not its own.

**A `ci` that ended `failure`, `cancelled` or `timed_out` now ends the wait green.** The step sets
`green=false`, writes a `::notice::` and a job summary, and exits 0. Every later step of the job
(signing material, bundles, uploads, checks, symbols) has `steps.ci.outputs.green == 'true'` as a
top-level `&&` term of its `if`, so it skips. `release-manifest` holds that for each step after the
wait, that the wait's id is the job's only `ci`, and that the wait has no `continue-on-error`.

**What the step cannot determine still fails the job:** `gh` errors after their retries, a run that
never ends or is never listed, and a conclusion that is neither green nor red (`skipped`, `neutral`,
`action_required`, `startup_failure`, `stale`). Those are faults, not a red commit. The publishing
jobs still need `gate` with `green == 'true'`.
