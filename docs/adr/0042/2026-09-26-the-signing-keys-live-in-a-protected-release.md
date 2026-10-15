# Amendment, 2026-09-26: the signing keys live in a protected `release` environment

Issue #344. The private key was a repository secret, which any workflow on any ref could read.
It is now a secret of a GitHub **environment** named `release`, and so are the Developer ID
secrets. The environment's deployment policy admits two refs:

- the branch `main`, because the dev channel publishes from a `workflow_run` job, and GitHub
  runs a `workflow_run` job on the default branch;
- tags matching `v*`, the stable channel.

`release.yml`'s `plan`, `build` and `publish` jobs name the environment whenever the run
publishes. `plan` reads only whether the secrets are set, but outside the environment it would
read "not set" and skip every dev build, so it names it too. A `workflow_dispatch` build
publishes nowhere, so it names no environment, holds no key, and is built without updater
artifacts, on any branch, `main` included. A test in `crates/release-manifest` holds every job
that reads `secrets.*` or publishes to the environment.

The operator creates the environment, moves the secrets and deletes the repository copies
(`docs/updating.md`, step 0). A repository secret of the same name would still reach a job
outside the environment. There is no required reviewer: one environment covers both channels,
so a reviewer would hold the dev build after every merge, once per job. The ref policy is the
guard, and a `v*` tag ruleset decides who can reach the stable half of it.
