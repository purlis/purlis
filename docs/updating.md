# How purlis updates itself, and the steps only the operator can take

purlis updates itself with Tauri's updater, from GitHub Releases, on one of two channels.
The reasons are ADR 0042. This page is the part a person has to do by hand: create one
environment, generate one keypair, store two secrets, create one release. Until they are done,
nothing is published and the app offers no updates. Every error the release workflow prints points back to one of
these steps.

**Steps 0, 1, 2 and 4 are required. Step 3 is not** — it buys a smoother first install and costs
an Apple Developer account. Without it macOS builds are ad-hoc signed, which publishes fine and
updates fine (ADR 0042 §3, amended 2026-09-23).

| Channel | Cut from | Manifest the app reads |
|---|---|---|
| **stable** (the default) | a `v*` tag, pushed by the operator | `releases/latest/download/latest.json` |
| **dev** | any green `main` | `releases/download/dev/dev.json` |

The two channels' builds differ in two things and no others (ADR 0092). A **dev channel build**
is compiled with the lighter `dev-release` profile and has no `.dmg`, which is what makes it
take about half the time; a first install of one on macOS uses `purlis-macos-arm64.app.zip`. A
**stable build** uses Cargo's `release` profile and has every bundle. Both are signed, checked
and built cold in the same way. `purlis --version` and About purlis say which profile a build
was made with.

Because the dev channel no longer builds the stable profile, a **profile guard** does: the
release workflow runs every night, builds `main` as a stable build is built, with no signing key,
and publishes nothing. If it fails it opens an issue titled *the stable profile does not build
on main*, and the first clean night closes it. Fix that before tagging.

### When a merge reaches the dev channel

A merge to `main` starts `ci` and a release run together. The release run waits for that
commit's `ci` to end (its `gate` job), and when it ends green, builds that commit and replaces
the dev release's assets. A dev build takes **about half an hour** after the wait: the Linux and
macOS builds run side by side for some twenty minutes, and the manifest, the provenance and the
publish take the rest. So a merge is on the dev channel about fifty minutes after its `ci` goes
green, when nothing is ahead of it.

The push starts the release run, rather than `ci`'s end, so that the run's own commit is the one
it builds: a run started by another run's end has `main`'s head at that moment as its commit,
and the build provenance it signs names that one, a later commit whenever a merge landed while
`ci` ran. Now the provenance names the commit built, and `gh attestation verify
--source-digest <commit>` checks it.

**Dev builds queue. A merge never costs the build in progress.** One release run on `main` runs
at a time and is never cancelled once it has started, so you can merge while one is running. The
next merge's run waits for it to finish. Only one run waits: if another merge lands meanwhile,
its run takes the waiting place and GitHub marks the one that was waiting *cancelled*. That one
had started nothing, and the run that replaced it is of a later commit and contains it. So
merging faster than a build takes publishes some commits and not each one. When the newest
merge's own `ci` then ends red, its run publishes nothing and the channel waits for the next
green merge. Every job of a release run has a deadline, so a hung build fails and frees the
queue.

A `ci` that ends red or cancelled publishes nothing: the gate says so in a notice and the run
builds nothing. A red `ci` run again by hand to green publishes nothing either; the next merge's
run does. The nightly `ci` starts no release run.

**The dev channel only moves forward.** Before it builds, a run reads the commit `dev.json`
names and publishes only a later one, so a run started again by hand days later says so in a
notice and builds nothing. The publish job asks again before it replaces the first asset, so
such a run fails there and replaces nothing.

In the Actions list a release run on `main` is titled `dev build of <sha>`, the commit it
builds.

A machine switches with `purlis update --channel dev` (or `stable`). The app checks a minute
after launch and every six hours after that. The week's first check reads the channel's weekly
manifest instead, which is how purlis counts weekly users without an identifier (below). It **installs only when asked**, because
installing restarts purlis and purlis owns every running session.

On macOS and Linux an installed update waits in place, and the title bar says **Restart to
update**. That writes down every chat and tab each open project holds, ends the chats, and
restarts into the new version, which asks whether to reopen every session or start fresh, says
that it restarted to install an update, and has **Reopen all** as the answer in front. A chat
that is mid-turn, or one that reports no state, is named first, and the operator chooses to
restart now or wait. A purlis started with `--no-restore` still puts its projects back after
the restart. On Windows the installer closes purlis itself when Install is pressed; purlis
writes the same records just before it does, and the installer starts it again. Nothing names
a mid-turn chat there first, and none of the Windows path has been run: nothing is ported to
Windows yet.

## 0. The `release` environment: where the secrets live

The signing secrets are **environment** secrets, not repository secrets. The release workflow's
`keys`, `build`, `sign-macos` and `publish` jobs (and the others that read a key) run in an
environment called `release` whenever they publish,
and the environment's deployment policy decides which refs may reach them:

| Policy entry | Type | Admits |
|---|---|---|
| `main` | branch | the dev channel: `release.yml` runs on each push to `main` |
| `v*` | tag | the stable channel: the operator's `v*` tag |

A job on any other ref is refused before it starts, so a pull request or a pushed branch never
sees a key. An on-demand `workflow_dispatch` build takes no environment at all: it publishes
nowhere and comes out unsigned for the updater.

```sh
gh api -X PUT repos/purlis/purlis/environments/release \
  -F 'deployment_branch_policy[protected_branches]=false' \
  -F 'deployment_branch_policy[custom_branch_policies]=true'
gh api -X POST repos/purlis/purlis/environments/release/deployment-branch-policies \
  -f name=main -f type=branch
gh api -X POST repos/purlis/purlis/environments/release/deployment-branch-policies \
  -f name='v*' -f type=tag
```

**The secrets have to be environment secrets only.** A repository secret of the same name is
still readable by any job outside the environment, a hand-started build included, so once the
environment holds them, delete the repository copies:

```sh
for s in TAURI_SIGNING_PRIVATE_KEY TAURI_SIGNING_PRIVATE_KEY_PASSWORD \
         APPLE_CERTIFICATE APPLE_CERTIFICATE_PASSWORD APPLE_SIGNING_IDENTITY \
         APPLE_API_ISSUER APPLE_API_KEY APPLE_API_KEY_P8; do
  gh secret delete "$s" --repo purlis/purlis 2>/dev/null || true
done
```

**The Report intake's upload secrets** (#5692) live in the same environment, so only a publish
reaches them. The `symbols` job sends each release's debug symbols to the intake (Sentry, EU),
after the build and apart from it, so native crash reports can be read. It needs three, which
the operator adds when the intake is set up (#5687). `tools/wizards/report-intake.sh` walks
through the whole setup, these four secrets and the steward's read token included:

| Secret | What it holds |
|---|---|
| `SENTRY_SYMBOLS_TOKEN` | a Sentry organisation token that may upload debug files (not the steward's read token) |
| `SENTRY_ORG` | the organisation's slug as Sentry gave it, `purlis-inc-ff` |
| `SENTRY_PROJECT` | the project's slug, `purlis-app` |

```sh
for s in SENTRY_SYMBOLS_TOKEN SENTRY_ORG SENTRY_PROJECT; do
  gh secret set "$s" --repo purlis/purlis --env release
done
```

**The Report intake's address** is a fourth secret in the same environment,
`PURLIS_REPORT_INTAKE_DSN`: the project's DSN (`https://<key>@<host>/<project>`), public by
design (ADR 0118). The `build` job hands it to every compile of a publish, stable and dev alike,
and purlis-core builds it in; a dev build's reports say `channel=dev`. Without it a build
still succeeds and sends no report at all.

```sh
gh secret set PURLIS_REPORT_INTAKE_DSN --repo purlis/purlis --env release
```

Until all three upload secrets are set, the `symbols` job skips with a note on the run and stays green. A failed upload
leaves the release published and the run green with a warning. Rerun the `symbols` job to try
again: the build keeps its symbols artifact for 90 days. The release profile writes line tables
into a packed dSYM beside each macOS binary and still strips the binary itself (ADR 0092,
amended 2026-11-10, #5692), since the native crash handler is on (#5691): the build saves the
dSYMs as the symbols artifact, and no bundle carries them. Linux and Windows follow their own
crash handlers.

**No required reviewer, on purpose.** The environment covers both channels, so a reviewer would
also hold the dev build after every merge to `main`, and it asks once per job: `plan`, each
platform's `build`, then `publish`. The ref policy is the guard. Add one in the environment's
settings if you want every publish approved by hand anyway.

**Who can push a `v*` tag** is a separate setting: a tag ruleset on `v*` restricted to the
repository's admins (Settings → Rules → Rulesets). The environment admits any `v*` tag, so the
ruleset is what keeps the stable channel yours.

## 1. The updater keypair (minisign): mandatory

This is the key the app checks every update against before it installs anything. Without it
there is no updater.

```sh
cd app
npx tauri signer generate -w ~/.tauri/charter-updater.key
#   (on a machine where npx hangs: node node_modules/@tauri-apps/cli/tauri.js signer generate -w ~/.tauri/charter-updater.key)
#   Give it a password when asked.

gh secret set TAURI_SIGNING_PRIVATE_KEY          --repo purlis/purlis --env release < ~/.tauri/charter-updater.key
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo purlis/purlis --env release   # prompts; paste the password
```

**Put the private key and its password in your password manager as well.** Every installed
purlis trusts this one key for good. If it is lost, no installed app can ever be updated again,
and every operator has to reinstall by hand. If it leaks, whoever has it can sign an update every
installed purlis will accept. Do not rotate it casually: an app only trusts the key it was built
with.

## 2. Commit the public half

```sh
jq --arg k "$(cat ~/.tauri/charter-updater.key.pub)" '.plugins.updater.pubkey = $k' \
  app/src-tauri/tauri.conf.json > /tmp/tauri.conf.json && mv /tmp/tauri.conf.json app/src-tauri/tauri.conf.json
git switch -c updater-public-key && git commit -am "The updater's public key" && gh pr create --fill
```

The `.pub` file is one line of base64, and that line is the whole value. A test in
`app/src-tauri` accepts either the placeholder or a real minisign key and fails on anything in
between. The release workflow refuses to publish while the placeholder is there.

## 3. The Developer ID certificate and notarization: optional, and what skipping them costs

**Skip this and everything still works.** With none of the six secrets below set, the
workflow ad-hoc signs the macOS bundle instead (`codesign -s -`). That is not the same as
leaving it unsigned: on Apple Silicon a binary with no signature does not execute at all, so
signing is never optional — only *whose* signature it carries is. What an ad-hoc signature
lacks is a developer for Gatekeeper to check, and the cost lands in exactly one place, a first
install from a browser, described at the bottom of this page.

**It does not cost you the updater.** The updater's trust is the minisign key from step 1, not
Apple's. Every update is verified against that key before anything is unpacked, whoever signed
the bundle.

### Where the key is used

The Developer ID private key is imported on a **GitHub-hosted** macOS runner and nowhere else
(ADR 0042, amended 2026-11-25). The `build` job may run on a self-hosted Mac (the repository
variable `MACOS_RELEASE_RUNNER`); it seals its bundle ad-hoc and never reads a secret below. The
`sign-macos` job, which runs on `macos-latest` after the checks have passed, in the `release`
environment, takes that bundle and signs it, notarizes a stable one, and makes the `.app.zip`,
the updater's `.app.tar.gz` with its minisign signature, and the `.dmg`.

- A **stable** build (a `v*` tag) is Developer ID signed **and notarized**.
- A **dev** build is Developer ID signed and **not** notarized: it reaches people through the
  updater, which sets no quarantine flag.

### The secrets

Six secrets, all **environment secrets of `release`** (step 0), in two groups. The Developer ID
group is all or none; the notarization group is all or none; and **a stable build needs both or
neither**. Half of either group, or a Developer ID without the notarization group on a stable tag
(or the reverse), stops the publish in the `keys` job, before the build, and says which is
missing. A dev build with the notarization group set does not notarize and says so.

| Secret | What it holds | Used for |
|---|---|---|
| `APPLE_CERTIFICATE` | the Developer ID Application certificate and its private key, as a `.p12`, base64 on one line | signing, every build |
| `APPLE_CERTIFICATE_PASSWORD` | the password the `.p12` was exported with | signing, every build |
| `APPLE_SIGNING_IDENTITY` | the certificate's identity string, `Developer ID Application: Purlis, Inc (TEAMID)` | signing, every build |
| `APPLE_API_ISSUER` | the App Store Connect issuer ID (a UUID) | notarizing, stable only |
| `APPLE_API_KEY` | the API key's **key ID** (ten characters, not the file name) | notarizing, stable only |
| `APPLE_API_KEY_P8` | the text of the `AuthKey_<KEYID>.p8` file | notarizing, stable only |

### How to produce each one

**`APPLE_CERTIFICATE` and `APPLE_CERTIFICATE_PASSWORD`: the Developer ID Application certificate
as a base64 `.p12`.** Only the account holder can create a Developer ID certificate.

1. On a Mac, open **Keychain Access → Certificate Assistant → Request a Certificate From a
   Certificate Authority**. Enter the account's email and a common name, choose **Saved to disk**,
   and save the `.certSigningRequest`.
2. On developer.apple.com: **Certificates, Identifiers & Profiles → Certificates → +**, choose
   **Developer ID Application**, upload the request and download the `.cer`. Double-click it to
   add it to the login keychain, the same one that holds the request's private key.
3. In Keychain Access, under **My Certificates**, find **Developer ID Application: Purlis, Inc
   (TEAMID)** with the key nested under it. Right-click it and **Export**, as `purlis-devid.p12`,
   with a password you will keep. Exporting the certificate without its key gives a `.p12` that
   cannot sign.
4. Then:

```sh
base64 -i purlis-devid.p12 | tr -d '\n' | gh secret set APPLE_CERTIFICATE --repo purlis/purlis --env release
gh secret set APPLE_CERTIFICATE_PASSWORD --repo purlis/purlis --env release       # the .p12 password
rm purlis-devid.p12                                                                 # and empty the Trash
```

**`APPLE_SIGNING_IDENTITY`: the identity string.** It is the certificate's name exactly as
`codesign` lists it, in the form `Developer ID Application: <Name> (<TEAMID>)`, where `<Name>` is
the legal name on the account and `<TEAMID>` is the ten-character team ID. With the certificate in
your keychain:

```sh
security find-identity -v -p codesigning
#   1) 0123456789ABCDEF0123456789ABCDEF01234567 "Developer ID Application: Purlis, Inc (ABCDE12345)"

gh secret set APPLE_SIGNING_IDENTITY --repo purlis/purlis --env release \
  --body "Developer ID Application: Purlis, Inc (ABCDE12345)"
```

Copy what is between the quotes, whole. The signing job refuses to start if the string matches no
identity in the `.p12`, and prints the identities the `.p12` does hold.

**`APPLE_API_ISSUER`, `APPLE_API_KEY` and `APPLE_API_KEY_P8`: an App Store Connect API key.**
Notarization signs in with an API key, never an Apple ID password.

1. On appstoreconnect.apple.com: **Users and Access → Integrations → App Store Connect API →
   Team Keys** (a *team* key, not an individual one). The account holder may have to request
   access to the API there first.
2. **Generate API Key** (or **+**), name it `purlis-notarization`, give it the **Developer**
   role, which is enough for `notarytool`, and generate it.
3. The **Issuer ID** is shown above the table of keys: a UUID. The **Key ID** is in the new key's
   row. **Download** the key: `AuthKey_<KEYID>.p8`. Apple offers the download once, so keep it
   until the secrets are set.
4. Then:

```sh
gh secret set APPLE_API_ISSUER --repo purlis/purlis --env release --body "00000000-0000-0000-0000-000000000000"
gh secret set APPLE_API_KEY    --repo purlis/purlis --env release --body "ABCDE12345"       # the key ID
gh secret set APPLE_API_KEY_P8 --repo purlis/purlis --env release < AuthKey_ABCDE12345.p8   # the file's text
rm AuthKey_ABCDE12345.p8                                                                    # keep one copy somewhere safe
```

The job writes the `.p8` to a file of its own for `notarytool`, with permissions for its owner
alone, and deletes it, and the keychain the certificate was imported into, when the job ends,
however it ended.

**If you adopt a team, keep it.** macOS lets an app replace its own bundle in `/Applications`
without asking for App Management permission when the new bundle is signed by the same team.
Changing teams is what makes every update start asking.

The obvious worry about ad-hoc is that it has no team at all, so nothing can match. **That was
measured on macOS 26.2 (Apple Silicon) rather than guessed, and it does not happen**: a replica
of the updater's install path, running inside an ad-hoc-signed app in `/Applications`,
replaced its own bundle with a differently-hashed ad-hoc one and was never asked for App
Management — `tccutil` afterwards reported no entry for the app at all. The run is described
in ADR 0042 §3, including what it does *not* cover. Moving from ad-hoc to a real team later is
the same one-time reinstall any team change is, so adopting a certificate later is not blocked
by anything here.

**Not set, on purpose:** `APPLE_ID` and `APPLE_PASSWORD`. Nothing here signs in with an Apple ID.

## 4. The dev channel's release: once

GitHub has no release without a tag, and nothing in CI creates a tag. So you create the one the
dev channel hangs on, once:

```sh
gh release create dev --repo purlis/purlis --prerelease --target main \
  --title "dev channel" --notes "The rolling dev channel. Not a release."
```

It **must** stay a prerelease. GitHub's `latest` pointer skips prereleases, and that is the only
thing keeping stable machines off dev builds. The workflow checks this before every dev
publish. After this, a green `main`'s build replaces this release's assets, one build at a time
("When a merge reaches the dev channel", above). The tag itself never moves.

## Building macOS on a self-hosted Mac

`release.yml`'s macOS build (the `.app`, and the macOS runner binary it hands to `runner-binaries`)
can run on a self-hosted Mac, **for a dev build only**: the dev channel's, started by a push to
`main` or by hand with the `dev-release` profile. A stable build (a `v*` tag) and the nightly
profile guard, which builds the way a stable build does, always run on `macos-latest`, whatever
the variable says. **A signed stable release must come from a GitHub-hosted runner**: a stable
build's key signs what a person installs and updates to, and the runner it came from is then
GitHub's to vouch for, not a laptop's. Only the build job reads `MACOS_RELEASE_RUNNER`, and only
when `plan` has chosen channel `dev` and profile `dev-release`; the Linux build and every other
job stay on GitHub's runners. A test holds that condition. The operator may overrule it, by
editing that line, which is a reviewed change.

**The variable** is a JSON array of runner labels, in Settings → Secrets and variables → Actions →
Variables (a variable, not a secret: it holds labels):

```sh
gh variable set MACOS_RELEASE_RUNNER --repo purlis/purlis \
  --body '["self-hosted","macOS","ARM64","purlis-release"]'
```

The workflow reads it as `fromJSON(vars.MACOS_RELEASE_RUNNER || '"macos-latest"')`, so an unset
variable is exactly the string `"macos-latest"`. **To go back to GitHub's runners, delete the
variable** (`gh variable delete MACOS_RELEASE_RUNNER --repo purlis/purlis`): no commit, and the
next run is hosted. Nothing in the code names a self-hosted label.

**One runner per Mac, with a label of its own.** `purlis-release` above is the dedicated label; do
not reuse `m1` or another shared one, or an unrelated job lands on the Mac. Register exactly one
runner service on each machine, and give each Mac's runner the same `purlis-release` label so
either can take the job. The build needs the whole 16 GB of a MacBook: #7499 measured 2.9 GB of
swap on a 7.5 GB runner. A second runner on the same Mac would run two builds at once and both
would swap. The job's `timeout-minutes` (150) is for a cold build on a hosted runner; time the
first one on the Mac before relying on it.

**What a persistent runner is made to forget.** A hosted runner is a fresh machine; a Mac is not,
so ADR 0092's rule (no cache in the job that holds the signing keys, ADR 0042) is kept by steps
that run only where `runner.environment` is `self-hosted`:

- the work folder is emptied before the checkout, so no `target/` or `node_modules/` of an earlier
  job survives;
- Cargo's, rustup's and the temporary folder are this job's own, under `$RUNNER_TEMP/job-home`, so
  neither `~/.cargo` nor `~/.rustup` nor `/var/folders` of the runner user is read, and the
  toolchain is installed per user from `rust-toolchain.toml` as on a hosted runner;
- the last step deletes every keychain in the runner user's search list that is not the login
  keychain, which is where the bundler imports a Developer ID certificate, even when the build
  failed. An ad-hoc build creates none.

The swap file, `sudo` and `apt` steps are Linux's; nothing for macOS touches system swap, Homebrew
or `/etc`.

**What the operator installs first**, on each Mac, as the runner user:

- the GitHub Actions runner, as a service of that user, with the label `purlis-release`;
- Xcode command line tools (`xcode-select --install`), and a signed-in GUI session for that user if
  a stable release is to make its `.dmg`, whose bundler drives Finder through AppleScript (it is
  allowed to fail; the `.app` does not need it);
- `gh` and `jq` at `/opt/homebrew/bin` or `/usr/local/bin`: the wait for `ci` takes `gh` from
  those two places only, and refuses another;
- `git`. Node and the Rust toolchain are installed by the job (`actions/setup-node`,
  `rust-toolchain.toml`), and `rustup` is used if it is there.

**The security preconditions.** Set all three before the variable. A runner that is online and in
no group of its own takes any job whose labels match.

1. **A runner group that only `release.yml` on `main` may use.** Organization Settings → Actions →
   Runner groups: put the two runners in a group of their own, restrict it to the `purlis/purlis`
   repository, and under "Workflow access" allow only
   `purlis/purlis/.github/workflows/release.yml@refs/heads/main`. A stable release starts from a
   tag and never takes the variable's runner, so the group needs no tag entry. A dev job that no
   runner of the group may take queues (GitHub gives up after 24 hours) and does not fall back to
   a hosted one.
2. **Fork pull requests need approval.** Settings → Actions → General → "Fork pull request
   workflows": require approval for all outside collaborators. No workflow a pull request starts
   may name a self-hosted runner, and `tools/no-self-hosted-on-pull-request.test.mjs` fails the
   build if one does (it also refuses a `runs-on` read from a variable, input or secret in one);
   this setting is the second line.
3. **A dedicated macOS user for the runner service.** A standard (non-admin) user that owns
   nothing else: no personal keychain, no SSH keys, no cloud credentials, no sudo. The job runs
   code from the commit it builds, as that user, and the keychain clean-up deletes that user's
   keychains.

**What provenance says of a self-hosted dev build.** It is attested and verified by the
`provenance` job, which runs on a GitHub-hosted runner, so `--deny-self-hosted-runners` stays on
both checks unchanged and passes: the flag looks at the runner that made the attestation, not at
the one that built the files. What the attestation then says about the *build machine* is the
operator's word, not GitHub's, and a compromised self-hosted Mac signs what it likes. That is why
a stable release never uses one, why the three conditions above come first, and why a self-hosted
run says so in its summary ("Built on a self-hosted runner").

## Cutting a stable release

What a release brought is written as it merges, one changelog fragment per pull request in
`changes/` (see `changes/README.md`), and folded into `CHANGELOG.md` (Keep a Changelog) under
`## [Unreleased]` when the release is prepared. Cutting the release is one PR and one tag:

```sh
# 1. one PR, merged:
#    - `node tools/changelog-fold.mjs`: every fragment in changes/ goes under `## [Unreleased]`,
#      first under its heading, and is deleted; the standing notice changes/STANDING.md goes
#      there too and stays
#    - `node tools/theme-parts-release.mjs`: the component parts manifest takes this release's
#      one version (ADR 0103, amended 2026-10-25): every `since: NEXT` becomes the last
#      version plus one, `parts::VERSION` is raised to it, its digest is pinned, and the theme
#      schema and docs/themes.md's family table written again. With nothing since NEXT it changes nothing. Its prose
#      it leaves: where the page says "the next release's manifest version" of what this release
#      numbers, write the number, or `every_version_the_docs_prose_gives_a_family_is_the_manifest_s_6308`
#      fails (#6308). The stable tag is
#      refused while a NEXT is left (`--check`)
#    - CHANGELOG.md: rename `## [Unreleased]` to `## [X.Y.Z] - YYYY-MM-DD`, put an empty
#      `## [Unreleased]` above it, and point the link references at the bottom at the new
#      tag (`[Unreleased]: …/compare/vX.Y.Z...HEAD`, `[X.Y.Z]: …/releases/tag/vX.Y.Z`)
#    - [workspace.package] version in Cargo.toml AND "version" in app/src-tauri/tauri.conf.json
#      set to X.Y.Z
#    - the browser engine's trust, refreshed (#2073, ADR 0042 amended 2026-10-16): Sigstore's
#      trusted root as its TUF repository serves it (`cosign initialize` fetches and verifies
#      it; copy ~/.sigstore/root/tuf-repo-cdn.sigstore.dev/targets/trusted_root.json) into
#      crates/browser-engine/sigstore-trusted-root.json, and the newest published
#      engine-manifest.json, once `cosign verify-blob` with that root, its bundle and
#      --certificate-identity https://github.com/purlis/purlis/.github/workflows/engine-manifest.yml@refs/heads/main
#      --certificate-oidc-issuer https://token.actions.githubusercontent.com accepts it, into
#      crates/browser-engine/engine-pin.json
git switch main && git pull
git tag vX.Y.Z && git push origin vX.Y.Z      # this, and only this, publishes a stable release
# 2. afterwards, bump both version fields to the NEXT version and merge, so dev builds are
#    X.Y.(Z+1)-dev.N
#
# Whenever the version fields change (step 2, the release commit, any bump), run
#    PURLIS_THEME_SCHEMA_WRITE=1 cargo test -p purlis-core the_published_schema
# in the same commit: it moves Purlis Classic's `$schema` to the new version's address, and
# `the_classic_folder_names_the_published_schema` fails until it has.
```

The `## [X.Y.Z]` section is the release: it becomes the GitHub release's body and the notes in
`latest.json`, and About purlis in that build shows the same section out of the copy compiled
into it. The release page puts **how to install** above it (`.github/release-install.md`, with the
tag and version filled in), because the page is where a first install starts. Lead the section
with a short paragraph saying what the release is, before `### Added`: that paragraph is the
headline on the release page and in About alike. So the workflow refuses a tag whose version has no section in `CHANGELOG.md`, or an
empty one, or whose `changes/` still holds a fragment nobody folded, before it builds anything,
beside refusing a tag that disagrees with `Cargo.toml`. To
see what a tag would publish, run `cargo run -p changelog -- X.Y.Z` on the merged `main`.

Between releases a test (`about.rs`) holds the crate version to the changelog: it has its own
section, or it is newer than every released version and `## [Unreleased]` is there. A test also
holds `tauri.conf.json` to `Cargo.toml`. A dev build shows `[Unreleased]` in About, as a dev build
of the next version, with the fragments in `changes/` folded into its copy before it is built
(nothing is committed); its release note stays the one-line "dev build of <sha>".

## What a first-time installer sees on macOS

**A stable build, with step 3 done, is notarized**, with the ticket stapled to the app and the
`.dmg`: Gatekeeper accepts a copy a browser downloaded, and the first launch needs nothing extra.

Everything else is **not notarized**, and a browser marks what it downloads as quarantined, so a
first launch is refused once. That is a dev build (Developer ID signed or ad-hoc), and any build
made with step 3 skipped. What differs is only how it is refused.

**The one instruction that works in every case**, before the first launch:

```sh
xattr -dr com.apple.quarantine /Applications/purlis.app
```

Quarantine is the flag Gatekeeper assesses. Remove it and there is nothing left to refuse.

**A dev build with a Developer ID (step 3 done).** macOS 15 and later answer a quarantined,
un-notarized app with **"purlis" Not Opened**, which offers only *Done* and *Move to Trash*.
Click *Done*, open **System Settings → Privacy & Security**, click **Open Anyway** beside purlis,
and confirm.

**Ad-hoc signed (step 3 skipped).** Gatekeeper has no developer to check, so it refuses a
quarantined copy outright. The **Open Anyway** route may work here too, but **that has not been
measured** and the dialog an ad-hoc bundle raises is not necessarily the one above — macOS has
a separate, blunter *"is damaged and can't be opened"* wording it uses for apps it cannot
attribute to anyone. Use the `xattr` command and the question does not arise.

**Updates do not meet any of this, ad-hoc included.** The quarantine flag comes from the
program that downloaded the file, not from macOS itself. purlis's updater downloads into
memory, verifies the minisign signature, unpacks the new bundle itself and renames it into
place, so nothing it writes carries the flag. Measured on macOS 26.2 (Apple Silicon) against a
replica of the updater's own install code, running inside an ad-hoc-signed app in
`/Applications`: a quarantine flag on the old bundle did not survive onto the new one, no App
Management permission was requested, and the replaced bundle launched and updated again. ADR
0042 §3 has the table and the limits.

## macOS privacy questions, and why purlis needs no Full Disk Access

macOS remembers what you allowed an app by the app's code identity. An ad-hoc signed build has
a new identity at every update, so macOS treats each update as a new app (#1654):

- **The Keychain asks again** for each item purlis keeps there, such as a 1Password vault's
  service-account token. The app reads each item at most once per run, when a chat or a
  vault's tab needs it, never to draw the Vaults panel. A 1Password vault's tab that the app
  reopens at launch waits until you press *Read* (#1660). *Allow* answers for that run.
  *Always Allow* answers for that build of the app, until the next update. Before reusing a
  token, the app checks that its Keychain item is still there without reading it, so a token
  you delete in Keychain Access is not used again.
- **App Data protection** ("would like to access data from other apps") should not ask about
  purlis's own work. The 1Password CLI read the 1Password app's settings, which macOS keeps in
  that app's container, on every run. purlis now tells it not to when it signs in with a
  service-account token or a 1Password Connect token, neither of which uses that app. A vault
  that signs in through the 1Password app still reads that app's data, because you chose that
  app.

**purlis needs no Full Disk Access, and you should not grant it.** macOS counts what a program
the app starts does as the app's own doing, so a grant to purlis reaches every chat it starts,
and a chat started without the sandbox reads with it. If **System Settings → Privacy &
Security → Full Disk Access** lists purlis (or its old name, charter), turn it off, and remove
the entries of builds you no longer run with **−**.

A stable code-signing identity (a Developer ID, step 3 above) is what stops both questions
coming back after every update.

## The `purlis` command in a terminal

The app ships its own command line as `purlis`, with `charter` beside it as an alias that runs
it, for the rename's window (RN-3). Every chat the app starts finds both first on its `PATH`. A
terminal does not, until they are put there:

- **macOS**: run **Install `purlis` command in PATH** from the command palette. It links
  `/usr/local/bin/purlis` and `/usr/local/bin/charter` to the `purlis` inside the app, the way
  VS Code's "Install 'code' command in PATH" does, and macOS asks once for an administrator's
  password when that directory is not yours. Links rather than copies, so the commands follow
  every update the app installs. A `charter` link an older app made is moved to the new
  binary. It never replaces a command somebody else put there — the Python charter, most
  likely — and says so instead, while still linking the other name; remove that one first if
  you want this one.
- **Linux**: the `.deb` installs `/usr/bin/purlis` and `/usr/bin/charter`. The AppImage runs from
  a mount point that changes at every launch, so there is nothing stable to link to.

## The runner image a release names

Each release builds purlis's runner image with its own version (`runner-image.yml`, called by
`release.yml`), and its manifest names it, beside `version`:

```json
"runnerImage": {
  "image": "ghcr.io/purlis/purlis-runner@sha256:<digest>",
  "signature": "<base64 minisign signature>"
}
```

The manifest itself is not signed, so the image's line is: the `runner-image-signature` job
signs it with the updater key (step 1), in the `release` environment, with the release's
version in the signature's trusted comment, as a bundle's is. `release-manifest` refuses a
release whose line does not verify against the public key the app ships, is signed for another
version, is not named by its digest, or is not purlis's own image.

A while after each launch, and as often as it looks for updates, the app reads its channel's
manifest for **its own** version and checks the image in it the same way. A Docker pool of
purlis's own image then gets a runner of that image beside its old one. New chats start on the
new runner, and the old one keeps its chats until they end, then is removed once every project its
chats ran for has brought back what they did there. Only a project open in a window does, so the
pool's row in Settings names each project the old runner waits on, says when a closed one was last
opened, and says that opening it, or removing the pool, lets the runner go. If it stops during that, it is
kept with its home. If it had stopped already, for a suspend, or its container was removed outside
purlis, it is kept only where its home holds a chat's conversation. Devices then offers to bring
those conversations back to the new runner, where their chats pick up again, or to discard them,
which asks twice. A pool of a team's
own image is never moved: its row in the window says its runner is behind. A manifest of another
version, or one from before the image was named, moves nothing.

**A runner image that fails never holds the desktop back.** The manifest, provenance and
publish jobs ask for the jobs the desktop needs by name, so a release whose runner image could
not be built or signed still ships the desktop, with no `runnerImage` in its manifest. Every
pool then stays on the runner it has, and the Pools group says a pool of purlis's own image does
not move to that version.

## Counting weekly users, without an identifier

purlis estimates how many machines use it each week from a download count, and from nothing
else (OB-17, ADR 0083 as amended for it). No install id, device id, hash or salt is sent, and
nothing is stored about anyone.

**What the app does.** Each release publishes its manifest twice, with the same bytes:
`latest.json` and `latest-weekly.json` on stable, `dev.json` and `dev-weekly.json` on dev. A
machine's first update check of each ISO week (UTC) reads the weekly file. Every other check
reads the usual one. The machine store notes the week (`machine.json`, `weekly`), so the next
check knows the week's count has been taken. That note stays on the machine, and every machine
that checked that week holds the same value. The request to GitHub is the same from every
machine: one URL with no query, and only fixed headers. They are `Host`, `Accept`,
`Accept-Encoding`, and a user agent that names the updater library's version, never the
machine's. A test, `no_machine_sends_anything_of_its_own_in_the_weekly_check`, runs three
machines with their own device ids. It checks that their weekly requests are byte for byte
identical and carry none of the ids.

**Why it cannot be linked.** The request carries no value of its own, so no value can match
from one week to the next or from one device to another. A per-week random salt or a hashed
install id would still send something that a log could join on. Here there is only a count.
GitHub can still connect one IP address's weekly fetch to that address's usual fetches. When the
weekly file is missing, the fallback also makes two requests back to back.

**Opting out.** Set `DO_NOT_TRACK=1` in the environment purlis starts with, and it never reads
the weekly file. The ordinary update check goes on. On macOS an app opened from the Dock or
Finder does not see variables set in a shell profile. Run `launchctl setenv DO_NOT_TRACK 1`
before opening purlis, or start purlis from a shell where the variable is set. A switch in
purlis's settings is tracked in #1185. Test builds and development builds make no
checks at all. A machine that cannot keep the note is never counted, because otherwise it would
be counted at every check: that means no config home, or a platform with no machine store.

**What GitHub sees.** GitHub serves the files, so GitHub sees each request's IP address and
user agent, as it does for every update check and download. That is GitHub's view, under
GitHub's privacy statement. purlis never receives an IP address: it reads only the per-asset
`download_count` that GitHub's releases API returns for every public repository.

**The estimator.** A download count only grows, so the estimate is a difference. Take two
listings a week apart:

```bash
gh api --paginate repos/purlis/purlis/releases > releases-2026-W41.json
cargo run -p release-manifest --bin weekly-users -- releases-2026-W41.json releases-2026-W40.json
```

The report lists each release's weekly-manifest count and the downloads of its bundles and
installers, with the growth since the earlier listing for each. Weekly users per channel are
**the sum, over that channel's releases, of the growth of the weekly-manifest count**. The
sum covers every release because stable's `latest/download` address hands the check to whichever
release is newest at that moment. A count therefore belongs to the release that was newest
then, not to the version the machine runs. Keep the listings yourself. purlis publishes no
estimate; the raw count is public, as every asset's is.

**Its biases:**

- **Undercount:** a machine that does not check during a week (switched off, offline all
  week, or on a build that makes no checks), or that has `DO_NOT_TRACK` set.
- **Overcount:** one machine with several OS users, or with a reset machine store, is counted
  once per store. A machine that changes channel in a week is counted on the first only. A
  clock that jumps between weeks can count a machine twice.
- **Reset counts:** replacing an asset starts its count again at zero. A stable release's
  manifests are replaced only when its publish is re-run. Dev's are replaced at every dev
  build, so dev's count covers only the time since its last build (the report prints the
  asset's upload time). Take dev's listings right before builds, or read its count as a lower
  bound.
- **Fallback:** a release published before the weekly manifest existed answers 404 for it.
  The check then reads the usual manifest, the week is not noted, and the next check tries
  again. Nothing is counted for such a release.
- **Anyone can inflate the count:** a request needs no identity, so anyone can fetch the weekly
  file in a loop. Read a sudden jump as suspect.
- **A counted request that then fails is counted again:** GitHub counts the download when it
  redirects to the file's storage host. If the request fails after that, for example behind a
  proxy that blocks that host, the week is given back and the next check counts again. That is
  up to about 28 times a week, one per six-hour check.
- **Not people:** a count is of machines that checked, and says nothing of who uses them.

## What a chat brings with it

Nothing has to be installed into Claude Code or Codex. The bundle carries its own Claude Code
plugin, `charter` (`Contents/Resources/plugin` on macOS, `/usr/lib/charter/plugin` on
Linux), and each Claude Code chat the app starts loads it for that session alone with
`--plugin-dir`: purlis's hooks, its Bash guard, and its skills, such as `purlis-handoff`,
`purlis-repo-work`, `purlis-secrets` and `purlis-browser`. The same chat turns the Python charter's
`charter@charter` plugin off for itself, so a plane whose settings enable that plugin for your terminal sessions does not give
an app chat two sets of hooks. A Codex chat is armed the same way with `-c` flags. A `claude`
or `codex` you run in a terminal is untouched, and an update to the app updates all of it.

## What an operator on Linux gets

The **AppImage** updates itself. A **`.deb`** does not: `dpkg` owns it, and the manifest
deliberately has no entry a `.deb` install would match. An Intel Mac and arm64 Linux get no
updates at all, because the release has no runner for them.
