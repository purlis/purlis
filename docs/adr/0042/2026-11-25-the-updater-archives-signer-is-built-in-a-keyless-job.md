# Amendment, 2026-11-25: the updater archive's signer is built in a keyless job of its own

The amendment of the same day, *Developer ID signs on hosted runners only, and a stable build is
notarized*, has `sign-macos` sign the updater's archive with `sign-updater-archive`, "built with no
key in reach, as `runner-binaries`' signer is". That compile was a step of `sign-macos`, and it
took about 11 of the job's 19 minutes (dev build run 45663417909). It could not start until
`build` had finished.

**The signer is compiled by a job of its own, `build-signer`, while `build` runs.** The job needs
`plan` and `keys`, never `build` or the gate. It runs on a literal GitHub-hosted `macos-` label
and never on `MACOS_RELEASE_RUNNER`. It takes no environment, names no secret, holds
`contents: read` and restores no cache. It uploads the one binary under one name, and gives its
sha256 as the job's output.

**`sign-macos` compiles nothing and runs only the bytes `build-signer` hashed.** It has no checkout
and no toolchain. It downloads the signer, compares the file's sha256 with `build-signer`'s
output, and fails before anything runs the signer if the two differ. The signature step runs the
signer from the path that comparison read.

What this keeps is unchanged: the signer is compiled from the run's commit, in the same run, on a
hosted runner, and no build script or proc macro of its dependency tree runs where the updater's
key is. What is new is that a binary crosses from one job to another before the key is used. The
sha256 comparison is what holds that crossing. No other job downloads the signer, by its name or
by a pattern that could match it.

`crates/release-manifest/tests/it/the_developer_id_key_is_imported_on_a_hosted_runner_only.rs`
holds each of these rules, and checks that each one refuses a copy of the workflow broken on
purpose.
