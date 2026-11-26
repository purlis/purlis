# Amended 2026-11-25: a dev build of macOS may run on a self-hosted Mac, and a stable build may not

**By the steward's ruling on the self-hosted macOS runner (option (a); the operator may overrule
it).** `release.yml`'s `build` job reads the repository variable `MACOS_RELEASE_RUNNER`, a JSON
array of runner labels, for its macOS leg, and only when `plan` chose channel `dev` and profile
`dev-release`. Unset, the leg is `macos-latest`, as before.

**What it does not change.** A stable build (a `v*` tag) and the nightly profile guard, which
builds the way a stable build does, never read the variable: a signed stable release comes from
a GitHub-hosted runner. Signing, the minisign key and the checks on the bundle are the same on
both channels, and `--deny-self-hosted-runners` stays on both provenance verifications: they
look at the runner that made the attestation, which is the hosted `provenance` job.

**What it adds to the "no cache" rule (ADR 0042, this ADR's decision 2).** A self-hosted Mac is
persistent, so the job empties its work folder before checkout, puts Cargo's, rustup's and npm's
homes and the temporary folder under `$RUNNER_TEMP`, deletes the keychains the signing leaves
and says in its summary that it ran self-hosted. Those steps run only where
`runner.environment` is `self-hosted`.

**What it costs.** What a dev build's attestation says about the build machine is the
operator's word. `docs/updating.md` ("Building macOS on a self-hosted Mac") holds the three
preconditions (a runner group limited to `release.yml` on `main`, fork-PR approval and a
dedicated macOS user) and `tools/no-self-hosted-on-pull-request.test.mjs` fails the build if a
workflow a pull request can start names a self-hosted runner.
