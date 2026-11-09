# Amendment, 2026-11-09: a dev build runs on its own push, after its own `ci`

A dev build was started by `workflow_run` when `ci` finished green on main, and the operator image
in the same way when `cluster operator` did. Under `workflow_run`, a run's commit (`github.sha`)
is main's tip when the run starts, not the commit it builds. The build checked out only the green
commit, so nothing unreviewed was ever built, but the SLSA provenance GitHub signs names the run's
commit. When main had moved in between, the provenance of a dev build, of the runner image and of
the operator image named a later commit than the one built, and a `gh attestation verify
--source-digest <commit>` of the built commit failed.

**A dev build now runs on the push to main itself.** `release.yml`'s first job, `gate`, waits for
that commit's `ci` push run and lets the build start only when it ended green. A red or cancelled
`ci` builds nothing, and a `ci` that cannot be found or does not finish in time fails the run. So
`github.sha` is the commit built, and every attestation verify pins `--source-digest` to it. The
operator image is published by `operator.yml` itself, which calls `operator-image.yml` once its
checks and kind tests passed on that push. A called workflow signs under its own name, so the
operator image's signing identity is unchanged.

**The release is not a job of `ci`.** Calling it from `ci.yml` would also make `github.sha` the
built commit, but a publish that failed (a registry or signing outage) would then turn main's
`ci` red, and a red `ci` on main stops all landing (`tools/land.mjs`). A workflow of its own that
waits keeps the two verdicts apart.

What it costs: each push to main holds a small runner while it waits for its `ci`. The release
keeps one waiting place, so a newer push replaces an older one still waiting; if the newer one's
`ci` then goes red, the channel waits for the next green merge. The check that skipped a build
whose `release.yml` differed from the running one goes: under a push the two are the same file.

Taken as the recommended option of the provenance review, under the operator's standing
instruction to build recommended options.
