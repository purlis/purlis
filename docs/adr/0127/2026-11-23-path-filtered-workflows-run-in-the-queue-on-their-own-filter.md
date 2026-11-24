# Amendment (2026-11-23, #7420): a path-filtered workflow runs in the queue on its own filter, or is kept on pull requests with a reason

`PR_ONLY` named thirteen path-filtered workflows that a pull request starts and the queue never
does. Each now has a decision, and the list holds nine.

- **Four join the queue, scoped by the filter they already have**: `docker-pool-hosts`,
  `one-login-e2e`, `one-login-proxy-listener` and `operator`. A merge group's event carries no
  `paths`, so each gains a `scope` job (`.github/actions/workflow-scope`, which runs
  `node tools/ci-scope.mjs workflow <file> <base>`) that reads **the workflow's own
  `pull_request.paths`** against what the group changes since its base, and its jobs run when the
  two meet. The filter is written once; the pull request's and the queue's cannot drift, and a
  change that cannot be read runs the workflow. `ci-stages.mjs` refuses a workflow that has both
  a `paths:` filter and a `merge_group` trigger without that step.
- **Nine stay on pull requests, each with its reason in `PR_ONLY`**, and a reason is at least 40
  characters: a label is refused. They gate nothing (`artifact-isolation`, `image-leftovers`), are
  a dry run of a publish whose real run is main's or the release's (`operator-image`,
  `runner-image`), watch paths of their own that no sibling change can break (`one-login-proxy-spike`),
  need what a merge group lacks (`stories-baselines`: a label; `codeql`: a branch its alerts
  belong to), cost a paid model's answers (`skill-grades`), or measure timing a shared queue runner
  would make flaky (`stress`).
- **No workflow was dropped from pull requests.** Each of the nine still earns its run there.
- **Two of the four have a macOS leg that the queue now runs when its paths change**:
  `one-login-e2e` and `one-login-proxy-listener`, one `macos-15` runner each. Macs are the queue's
  scarce resource (the amendment of this day on `outcome-bars`); a queue run that touches
  neither's paths starts neither. Reverse by taking the workflow's `merge_group` trigger and its
  `scope` job off and listing it in `PR_ONLY` with the reason.
