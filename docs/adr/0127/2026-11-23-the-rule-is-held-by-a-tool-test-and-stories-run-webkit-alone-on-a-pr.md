# Amendment (2026-11-23, #7335): the rule is held by a tool test, and stories run WebKit alone on a pull request

The first step of the ready gate (spec #7324, train B) landed in `tools/ci-scope.mjs`, which takes
the pull request's draft state, and in `tools/ci-stages.mjs`, which reads the workflows.

- **A draft runs no heavy scope**: `rust`, `app`, `scenario`, `stories` and every gated job are off
  on a draft, a change to `.github/` included. Marking it ready (`ready_for_review`, which `ci.yml`
  and `stories.yml` trigger on) runs the change's scopes, and a change to the workflows runs every
  one. A merge group, a push and a nightly run every scope.
- **`ci-stages.test.mjs` holds "a PR run is a subset of the queue run"** against the workflows: a
  job that can run on a pull request and cannot on a merge group fails it, and so does a workflow
  that triggers on a pull request with no `merge_group` trigger unless `PR_ONLY` lists it with its
  reason. `PR_ONLY` names thirteen path-filtered workflows (the rule's gap before this) so that a
  fourteenth is a choice.
- **Stories run WebKit alone on a ready pull request, and both browsers in the queue.**
  `stories.yml` gained its `merge_group` run. A plain ready pull request therefore draws and holds
  only WebKit's baselines, **but a pull request that asks for baselines (the `update-baselines`
  label) runs Chromium as well**: the queue draws no baselines, so its own run is the only place a
  Chromium picture can be taken. `labeled` starts that run when the label is put on after the last
  push. A picture that differs in Chromium alone, with no label, first fails in the queue. If that
  proves common, the move reverts under the rule above (queue drop rate more than 5 points over
  15.3% seven days after the merge).
- **macOS `outcome-bars` left the pull request run** (the steward's ruling, 2026-11-23). It ran on a
  pull request and on main's push but not in the queue, which the rule forbids. Macs are the
  queue's scarce resource, so it did not join the queue: it now runs on main's push and the nightly
  alone. Its Linux leg is unchanged.
