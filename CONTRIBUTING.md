# Contributing to Purlis

Thank you for your interest in contributing to **Purlis**!

Purlis is an open-source project focused on making agentic software development more effective, observable, and secure. We welcome bug reports, documentation improvements, tests, integrations, and well-scoped code contributions.

This guide explains how to participate and what to expect when contributing to the official project.

## 1. Ways to contribute

You can help by:

- Reporting reproducible bugs or usability problems.
- Suggesting features or improvements to developer and agent workflows.
- Improving documentation, examples, and onboarding.
- Fixing bugs, improving tests, or addressing performance issues.
- Proposing integrations with coding agents and development tools.
- Improving observability, permissions, security, and auditability.

Contributions of any size are appreciated. Maintainers decide which changes fit the project's roadmap and architecture.

## 2. Before you start

**For small fixes:** You can open a pull request directly.

**For substantial features, architecture changes, or new integrations:** Please open a GitHub issue first and describe the problem, proposed approach, and alternatives. This helps avoid work that conflicts with the project's direction.

Before opening a new issue or pull request, check whether a similar discussion or change already exists.

## 3. Reporting bugs and requesting features

Open a GitHub issue with enough detail for someone else to understand or reproduce the problem.

For a **bug report**, include:

- What you expected to happen and what actually happened.
- Clear steps to reproduce the issue.
- Relevant versions, operating system, and environment details.
- Minimal logs or screenshots, **with secrets and personal data removed**.

For a **feature request**, include:

- The problem or user need.
- Your proposed solution and why it helps.
- Any alternatives or trade-offs worth considering.

**Security vulnerabilities must not be reported in public issues.** See [Security reporting](#7-security-reporting).

## 4. Development and pull-request workflow

1. Fork the repository and create a topic branch from the repository's default branch.
2. Follow the local development and testing instructions in the project's `README.md` and other documentation, where available.
3. Keep changes focused. Avoid mixing unrelated refactoring with a bug fix or feature.
4. Add or update tests when changing behavior, and update documentation when needed.
5. Run the relevant tests, formatting, and lint checks for the areas you changed.
6. Submit a pull request to the project's default branch with a clear description.

In your pull request, explain:

- **Why:** What problem does this change solve?
- **What:** What changed, and what important design decisions were made?
- **Validation:** What tests or manual checks did you run?
- **Impact:** Are there compatibility, security, performance, or migration considerations?
- **Related work:** Link any relevant GitHub issue or discussion.

Draft pull requests are welcome for early feedback. Maintainers may request revisions, suggest a different approach, or decline a change based on project priorities.

### Building and testing

[README.md](README.md#develop) lists what to install and every check CI runs. Run them before
you push:

```bash
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings    # the core and the command line
cargo test
cargo deny check                             # licences and advisories

cd app
npm ci
npm run typecheck && npm run lint && npm run format:check && npm test
```

A change to recorded behaviour or the command line also runs
`cargo test -p purlis-cli --test recorded_behaviour`. The website and docs are checked with
`cd site && npm ci && npm test && npm run build`.

CI runs all of these on every pull request: formatting, clippy with `-D warnings` on every
crate (the app crate included), the Rust tests, the licence and advisory check (`cargo deny`
and `tools/npm-licences.mjs`, against the one allow-list in `deny.toml`, and
`tools/third-party-notices.mjs`, which fails when a crate or npm package the app ships has no
licence text for the notices a build generates), the app's typecheck,
lint, formatting and tests, the scenario tests against a built app, and the docs site's build
and link check.

The scenario tests (`app/e2e`, run with `npm run build:e2e` and then `npm run e2e`) are
reliable on CI. On a busy machine they are not, and CI is the reference. With the load around
20 on macOS, every wait for a menu, a dialog or the ⌘P scope to be _displayed_ can time out,
and that includes tests that pass on CI. If a local run fails like that, check the load before
you debug the spec. In a spec, a menu that only has to exist before it is used is waited for
with `waitForExist`, not `waitForDisplayed` (#1152).

[CLAUDE.md](CLAUDE.md) lists the rules that are easy to break, such as keeping the core free of
the UI and changing a recorded answer only on purpose. The specification is
[`docs/spec.md`](docs/spec.md), and [`CONTEXT.md`](CONTEXT.md) defines the words purlis uses.
Every first-party source file starts with its copyright and licence lines
(`Copyright 2026 Purlis` and `SPDX-License-Identifier: Apache-2.0`, after a shebang line), and a
test fails a file that lacks them.

### A test with every change

A pull request comes with a test that fails without the change and passes with it, named for
the behaviour it checks. When the pull request meets an issue's acceptance lines, it names the
test behind each one.

A change to what the window draws is done only once someone has looked at a screenshot of the
real window, in the light and the dark theme, and narrow and wide where that matters. Attach it
to the pull request, or say what it showed: the app's tests draw no pixels.

### Where tests live, and what a scanner's finding gets

A Rust module's tests are in a file of their own, `<module>/tests.rs` beside `<module>.rs`,
declared with `#[cfg(test)] mod tests;`. A module that is a folder keeps them in its `tests.rs`
or under its `tests/`. A window test is a `*.test.ts` or `*.test.tsx` file. Code scanning tells
test code from shipped code by where the file is, so a test written inside a shipped file is
read as shipped code, and its fixtures are reported as if a build carried them. A new module
starts this way; an older `mod tests { … }` block or `<module>_tests.rs` file moves when its
module is next changed, as a move alone, with no test's name or body changed.

A code-scanning finding is answered in the code before anything else. Where the code is right
and only reads wrong, make what it means explicit: a name that says what the value is (a key
file's name is not a key), a type, a documented constant, or the standard helper in place of a
hand-written one. A finding is dismissed only when no honest change removes it, and the
dismissal says why. Code is not bent out of its natural shape to get past a rule, and a rule is
not switched off for shipped code.

### Changelog fragments

When people using purlis would notice the change, add a changelog fragment,
`changes/<slug>.md`. It holds a `### Added`, `### Changed`, `### Fixed` or `### Security`
heading (or another of Keep a Changelog's) and the entry under it, written as it will read in
[CHANGELOG.md](CHANGELOG.md). Don't edit CHANGELOG.md itself: release prep folds the fragments
into it, so pull requests never conflict there. [`changes/README.md`](changes/README.md) has an
example.

### Architecture decision records

A change to a decision that is already recorded goes through a new record in
[`docs/adr/`](docs/adr/). Read the records near what you are changing first: they say why
things are the way they are.

### How maintainers land work: trains

A pull request from outside the maintainers is reviewed and merged on its own, as above.
Maintainers batch work into **trains**, so CI runs once for many tickets instead of once per
ticket:

- **A ticket is a branch, not a pull request.** Push the ticket's branch
  (`git push -u origin <branch>`) and open no pull request for it. A branch with no pull request
  starts no CI: `ci.yml`, `docs.yml` and `stress.yml` run on `pull_request` and on a push to
  `main` only.
- **One commit per ticket.** Squash the ticket's work into one commit: a title line, a body,
  `Closes #<n>` (or `Refs #<n>` when not every acceptance line is met) and any trailers,
  including its `Signed-off-by` line. Rebase it onto `origin/main` before handing it over.
- **Run the checks locally before pushing**, because nothing runs them for a ticket branch:
  `cargo fmt --all --check`, clippy with `-D warnings` on the crates you touched, and the test
  files you touched or added. For app changes, also `npm ci`, then typecheck, lint, format and
  the vitest files you touched. A change to recorded behaviour or the command line also runs
  `cargo test -p purlis-cli --test recorded_behaviour`.
- **A train is one pull request, `train/<date>-<n>`** (for example `train/2026-10-03-1`), that
  stacks the tickets' commits on `main`. CI runs on it once, and it is merged **by rebase**, so
  each ticket stays one commit on `main` and its `Closes #<n>` closes its issue.

### Releases

The maintainer cuts every release and is the only one who pushes a `v*` tag. Before a version
is promoted from the dev channel to stable, the maintainer opens a
[release checklist](https://github.com/purlis/purlis/issues/new?template=release.yml), one
issue per release, and checks every box: the technical gate (CI, the nightly, signing,
provenance and the SBOM), at least 3 days of the maintainer running it as their daily app on
the dev channel, the user signal, the test behind each acceptance line the release meets, the
outcome bars of the features it ships, the folded changelog and its news entry. Each box names
where its evidence is read. The form is
[`.github/ISSUE_TEMPLATE/release.yml`](.github/ISSUE_TEMPLATE/release.yml), and
`node tools/release-form.mjs` fails when it loses a gate.

## 5. Quality and design expectations

We value maintainable solutions over unnecessary complexity. Please aim for:

- Clear, readable code consistent with nearby project conventions.
- Small, reviewable changes with appropriate test coverage.
- Useful error handling, documentation, and backward compatibility where feasible.
- Minimal dependencies; discuss significant new dependencies before introducing them.
- Respect for user privacy and secure defaults.

### Commands never block the window

A Tauri command declared as a plain `fn` runs on the app's main thread, the one that draws the
window: while it works, nothing is drawn. So a command that touches a file, runs a process
(`git` included) or waits on a lock another purlis may hold is `async` and does its work in
`crate::off_the_window` (`app/src-tauri/src/off_the_main_thread.rs`). A synchronous command is
allowed only for what the app holds in memory, or where the window relies on the order its
asks land in (a terminal's input, a tab moved); each is listed with its reason, and a test fails
on a new one that is not. A command made `async` answers when it is done: where the window both
reads and changes the same thing, put the commands in one lane of `app/src/commandOrder.ts`. A
new call that writes a file a lane reads fails a test until `app/src/writeSites.json` names it,
with the commands that reach it, which then go in that file's lane or out of it with a reason.

### AI-assisted contributions

Using AI coding tools is welcome, but **you remain responsible** for everything you submit. Review and test AI-generated changes, check for insecure behavior or leaked secrets, and ensure you have the rights to contribute all included code and materials. Mention substantial AI assistance in the pull-request description when it is relevant to review.

### Security-sensitive changes

Changes involving agents, command execution, permissions, credentials, secrets, remote development environments, networking, or audit logging deserve extra care. Where relevant, describe:

- Trust boundaries and the permissions an agent or user will receive.
- How access is limited, approved, logged, and revoked.
- Potential failure modes, abuse cases, and mitigations.
- How sensitive data is protected in logs, tests, and error messages.

Do not commit real credentials, tokens, customer data, or other confidential information.

## 6. Licensing and contribution certification

Purlis is distributed under the **Apache License, Version 2.0**. By intentionally submitting a contribution for inclusion in Purlis, you agree that it will be submitted under the Apache License 2.0, unless you explicitly state otherwise and the maintainers agree in writing. See the repository's [`LICENSE`](LICENSE) file.

You must have the legal right to submit your work. Preserve applicable third-party copyright and license notices; do not copy code from sources with incompatible or unknown licensing.

### Developer Certificate of Origin (DCO)

To document the right to contribute, **each commit must include a `Signed-off-by` line**. You can add this automatically when committing:

```bash
git commit -s -m "Describe your change"
```

The sign-off should use your real name and an email address you control, and must match the author identity you are entitled to use. By signing off, you certify that the contribution complies with the [Developer Certificate of Origin, version 1.1](https://developercertificate.org/).

If you are contributing on behalf of an employer, make sure you have any necessary authorization.

## 7. Security reporting

Please **do not disclose potential security vulnerabilities in public GitHub issues, discussions, or pull requests**.

If the repository has GitHub's **Private vulnerability reporting** enabled, use the **Security** tab to submit your report privately. Otherwise, contact the project maintainers privately through a contact method published in the repository. If you cannot identify a private reporting channel, ask maintainers how to report securely **without sharing vulnerability details publicly**.

Include a description, affected components, impact, and reproduction steps where possible. Do not include real credentials, personal information, or customer data.

Purlis has private vulnerability reporting enabled: [report a vulnerability](https://github.com/purlis/purlis/security/advisories/new) from the **Security** tab. [SECURITY.md](SECURITY.md) has the details, and how to check that a download is a real Purlis build.

## 8. Community conduct

Be respectful, constructive, and professional. Discuss ideas and technical decisions rather than attacking people. Harassment, discriminatory conduct, and abusive communication are not welcome.

Maintainers may moderate discussions and contributions to keep the project collaborative and productive.

Everyone taking part follows the [Code of Conduct](CODE_OF_CONDUCT.md), which also says how to report a conduct problem.

## 9. Review and project governance

- Opening a pull request does not guarantee it will be merged.
- Maintainers have final responsibility for technical direction, releases, and decisions about contributions to the official repository.
- Contributions may require changes before acceptance, including changes for security, maintainability, or licensing reasons.
- Contributions do not confer ownership or decision-making authority over the project or its trademarks.

Thank you for helping build Purlis!
