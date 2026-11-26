# Amendment, 2026-11-25: Developer ID signs on hosted runners only, and a stable build is notarized

purlis/purlis now has an Apple Developer account (Purlis, Inc). Two of the operator's rulings of
2026-11-25 follow from it, and each changes a thing this record said.

**Developer ID signing happens on a GitHub-hosted macOS runner and on no other.** Since #7641 the
`build` job's macOS leg may run on self-hosted Macs (the repository variable
`MACOS_RELEASE_RUNNER`). A Developer ID private key imported on a machine the operator or a
contributor runs is a key on a machine this workflow does not control, so it is never imported
there.

- **`build` signs nothing with a Developer ID, on any runner.** It compiles, bundles and seals the
  macOS bundle ad-hoc (`APPLE_SIGNING_IDENTITY=-`), which it always did for a build that publishes
  nothing, and it reads no Developer ID, notarization or macOS updater secret. Its macOS leg makes no
  updater archive, so the updater's minisign key is not read on that runner either (the Linux leg
  still reads it, as before). On a run that publishes it uploads the ad-hoc bundle as
  `purlis-macos-arm64-unsigned-<version>`, a name nothing that publishes downloads.
- **A new job, `sign-macos`, signs it.** It runs on `macos-latest`, a literal hosted label and never
  a variable, behind the gate, in the `release` environment, and only on a run that publishes. It
  takes the bundle `build` made and signs every nested Mach-O file and bundle, deepest first, then
  the app, with the hardened runtime and a secure timestamp. **It passes no entitlement**: the app
  carries none (ruling V90a, `the_app_keeps_the_hardened_runtime.rs`), so "the app's entitlements"
  is the empty set and the job holds the app to it. It then makes what `build` made after signing
  before: the `.app.zip`, the updater's `.app.tar.gz` and its minisign signature, and the `.dmg`
  (made with `hdiutil` from the signed app, in place of Tauri's Finder-driven bundler, so its window
  is the plain one). It uploads them as `purlis-macos-arm64-<version>`, the name `sbom`, `manifest`,
  `provenance` and `publish` already downloaded, and each now needs it. The signature is made by
  `sign-updater-archive`, a binary of `release-manifest` built with no key in reach, as
  `runner-binaries`' signer is.
- **With no Developer ID secrets the job seals the bundle ad-hoc again**, so ad-hoc publishing
  still works as the amendment of 2026-09-23 says, and the job still makes the archive and its
  signature.
- **The verification moves to the final artifact and stays on the one `build` has.** `build` asserts
  that its bundle is ad-hoc and holds no team, so a Developer ID that reached it is a red job.
  `sign-macos` unpacks the `.app.zip`, the `.app.tar.gz` and the `.dmg` it made and holds each to
  the signing that was asked for: `codesign --verify --deep --strict`, the Developer ID authority, a
  team and a secure timestamp on the app, its sidecars and every nested executable (or
  `Signature=adhoc` and no team, ad-hoc), and one code directory hash across all of them and the
  app it signed. The hardened-runtime and no-entitlement read of ruling V90a is made of the shipped
  app, and of every nested executable, with the exemption it had (report-only on a dev publish,
  blocking on a stable one), no wider.

**A stable build is notarized, and a dev build is not.** The amendment of 2026-09-23 declined
notarization because it needed an Apple Developer account, which the project did not have. It has
one, so a stable build is notarized with an App Store Connect API key, the way Tauri and
`notarytool` both read one: `APPLE_API_ISSUER`, `APPLE_API_KEY` (the key id) and `APPLE_API_KEY_P8`
(the `.p8`'s text), secrets of the `release` environment, read by `sign-macos` alone. The job
writes the `.p8` to a file of its own with `umask 077`, submits the app as a zip, waits (forty
minutes at most, fifteen for the `.dmg`), prints Apple's log and fails the job on any answer but `Accepted`, staples the
ticket into the app before the archives are made, and does the same for the `.dmg`, which a stable build must have: without one it is not published. It then checks
`stapler validate` and Gatekeeper's `Notarized Developer ID` on the final artifacts, and removes
the key file and the keychain in a step that runs however the job ended. A dev build is Developer
ID signed and **not** notarized: it reaches people through the updater, which downloads into memory
and sets no quarantine flag, so notarizing every merge would add Apple's waiting time to each one for a check no
Gatekeeper makes.

**All or none, loudly, before the build.** The Developer ID is three secrets and notarization is
three more. `keys` counts each group as a boolean and refuses a publish when either is half set. A
stable build is then one of two things: ad-hoc, with none of the six, or Developer ID signed and
notarized, with all six. A stable build with a Developer ID and no notarization key, or the key and
no Developer ID, is refused with the one thing to set. A dev build with the whole key notarizes
nothing, and the job's summary says so. This changes one behaviour: a stable tag that was Developer
ID signed and un-notarized now needs the notarization secrets set first
(`docs/updating.md`, step 3).

**The residual risk, plainly.** The key is no longer on a runner `build` was given, and the bundle
still is. `sign-macos` signs, and a stable build notarizes, whatever `build` put in the bundle, and
`build` may run on a self-hosted Mac. A compromised self-hosted runner cannot take the key, but it
can hand over a tampered bundle, and that bundle is then Developer ID signed and, on a stable
build, notarized. What the workflow checks of the bundle it is handed is that it is `build`'s own
kind: sealed ad-hoc (`codesign --verify --deep --strict`, `Signature=adhoc`), with the sidecars and
the built-in extension where the build checks expect them. It cannot check that no code was added.
Trust in a self-hosted runner is the trust in its bundle's bytes, and #7641's variable decides which
runners get that trust.

**What does not change.** The updater's trust is the minisign key, not Apple's (§3 of the
amendment of 2026-09-23). Signing is still never skipped, only downgraded. The signing keys are read
only in the `release` environment and only by jobs behind the gate or after the wait, and no job that
holds a secret restores or saves a cache. The compile still happens before the gate and every step
that signs or uploads still comes after it (the amendment of 2026-11-23): `sign-macos` is one more
job behind the gate. Team continuity (§3) still holds: adopting the Developer ID is the one-time
reinstall any change of team is.

`crates/release-manifest/tests/it/the_developer_id_key_is_imported_on_a_hosted_runner_only.rs` holds
the rules, each on a workflow broken on purpose: only `sign-macos` reads or names a Developer ID or
notarization secret, it runs on a literal hosted `macos-` label with no matrix, container or
variable, it needs the gate and runs in the `release` environment only when the run publishes, it
signs inside out with the runtime and no entitlement, the final artifacts are read before they are
uploaded, and notarization is a stable build's, with the whole key. `tools/release-keys.test.mjs`
runs the `keys` job's shell for each combination of the six secrets.

What it costs: a publishing run has one more job on the longest chain, a hosted macOS runner for a
few minutes (and the time Apple's notary service takes on a stable build); the `.dmg` loses the
styled Finder window Tauri's bundler scripted; and a stable tag needs six Apple secrets, or none.
