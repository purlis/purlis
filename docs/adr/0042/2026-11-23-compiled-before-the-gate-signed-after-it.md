# Amendment, 2026-11-23: compiled before the gate, signed after it

A dev build waited for its commit's `ci` before it did anything, so its 70 to 110 minutes of
compiling started only when `ci` had ended. Nothing in the compile needs to wait: it reads the
commit's own checkout and the lockfiles, and it signs, uploads and publishes nothing.

**The build now starts with the push and waits for `ci` only before it signs or uploads.** The
`build` job and `runner-binary-aarch64` compile at once. Just before their first step that signs
or uploads, a step waits for the commit's `ci` on `main`. The step is written out in the workflow,
as `gate`'s is, and no file of the checkout runs it: the commit's code has already run in that job.
It runs in the runner's own bash with `BASH_ENV` unset, takes a fixed `PATH` and refuses a `gh`
that is not the runner's own. It drops the environment the compile left in `$GITHUB_ENV` (`GH_HOST`,
a proxy, `LD_PRELOAD`) bar what it needs, and reads status and conclusion in one call, tried three times, so an
API error is not an answer and a cancelled `ci` still ends the job. A red or cancelled `ci`, or one
that never ends, ends the job there, with nothing
signed and nothing uploaded, and nothing downstream publishes. A tag, a build by hand and the
nightly pass at once, as they pass `gate`.

**Every other job still needs the gate and runs only on its green:** the runner image and its
signature, the runner binaries, the SBOM, the manifest, provenance, publish and the symbols. Each
job that holds a write permission or `id-token` is one of them, and so is every job that uploads
or publishes. `build` and `runner-binary-aarch64` are the two with the environment that start
before the gate. `keys` is the one other job with the environment that is not behind it, and it
reads booleans only (`secrets.X != ''`), runs nothing from the commit and holds `contents: read`.
`symbols` also runs only when `keys` succeeded.

**The residual risk, plainly.** `build` and `runner-binary-aarch64` are in the `release`
environment from their first step, so the hosted runner holds that job's secrets while the commit's
code (a build script, a proc macro, an npm script) compiles before the wait. That code can read the
job's environment and anything the runner holds. The same commit's code already ran in that job
with those secrets after the gate, so a green `ci` is no trust signal for what that code does, and
this change widens nothing: it moves when the code runs and not whether it runs. The job keeps
build and sign in one job (ADR 0092), so the signing keys are in its reach once the wait has
passed and the compile reads one secret before it, the Report intake's address, which the binary
carries in the clear. What the rules hold is narrower: nothing before the wait names a signing
secret, signs or uploads, and the wait cannot be turned off by an edit to the workflow without a
rule failing. Code that ran in the job can still tamper with the runner it runs on, and the
wait's checks make that harder and do not make it impossible.

**`plan` leaves the `release` environment and holds no secret.** It runs the commit's own tools,
so it takes no environment and does not wait for the gate. Whether the updater key and the Apple
certificate are set is asked by `keys`, a job that checks nothing out, runs nothing from the commit
and reads each secret only as a boolean. The decisions that followed from those answers are the
same: a dev build with no updater key is skipped with a warning, a tag with none is an error, a
half-set Developer ID is an error, and a build with no Developer ID is ad-hoc signed.

What does not change: the signing keys are read only in the `release` environment and only by jobs
behind the gate or after the wait; no job that holds a secret restores or saves a cache (the
amendment of 2026-09-29); every bundle is built from the job's own checkout (ADR 0092); and
nothing is published for a commit whose `ci` did not end green. `crates/release-manifest` holds the
rules: `plan` takes no environment, the wait comes before any step that signs or uploads, every
other job needs the gate, and each fails on a workflow broken on purpose.

What it costs: a red `ci` now costs a compile's runner time before the build ends, where it cost
none, and a red main shows as a red build job, not a skipped run.
