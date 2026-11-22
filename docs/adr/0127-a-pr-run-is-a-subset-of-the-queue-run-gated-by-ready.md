# A PR run is a subset of the queue run, gated by ready

**Accepted 2026-11-21**: decided in the CI distribution grill (spec #7324, 20 questions over three
rounds). It links [ADR 0042](0042-charter-updates-itself-and-nothing-it-cannot-verify-reaches-it.md)
(the main run stays, because the dev release waits for it) and
[ADR 0086](0086-every-performance-budget-names-the-job-that-measures-it-and-ci-holds-charters-own-cost-by-regression.md)
(every budget names the job that measures it).

## Where CI was

Pull requests took 90% of runner time: 21,113 of about 23,500 job-minutes over 250 runs on
2026-11-20, because every push to a PR ran nearly every suite. The queue took 8%, the main run 2%.
Meanwhile queue drops came from flakes and infra, not from code, and each drop rebuilds everyone
behind it at about 35 to 40 minutes. From 2026-11-14 to 11-21 the ready-to-merged median was
291 minutes (p90 1,655) and the queue dropped 20 of 131 entries, **15.3%**.

## The decision

**Each job belongs to one or more stages (PR run, queue run, main run, nightly run), and a PR run
never runs a job the queue run lacks.**

- A draft gets the fast guards and web tests only.
- A **ready gate** splits the PR run: a ready PR also gets the heavy jobs whose change scope
  fires. Any change to `.github/` or `ci/` turns every scope on.
- The queue run runs everything, including both story browsers and macOS scenario.
- The nightly run adds Windows, Fedora, macOS tests and the quarantined tests.
- The main run stays as it is: the dev release waits for it (ADR 0042), and red-main detection
  reads it.

**A flake is recorded and never silently green.** A flake is a test that failed, then passed on
its one retry in the same run.

- Every suite retries a failed test once: nextest (`retries = 1`, profile `ci`), vitest
  (`retry: 1` when `CI` is set) and wdio (`specFileRetries: 1` in CI). A pass on retry writes one
  annotation, `Flaky (passed on retry N): <test>`, which `tools/ci-health.mjs` counts
  (`tools/flake-annotation.mjs` is the one writer).
- The daily ci-health run files a ticket per flake.
- A test that flakes three times in seven days is quarantined to the nightly run, in
  `ci/quarantine.toml`, until its fix lands. At most 10 tests can be quarantined at once.
- Each job's p50 has a budget in `ci/job-budgets.toml`, which ci-health checks, in line with
  ADR 0086.

## Considered

- **Run everything on every PR push** (today's setup): it costs the most, and macOS PR runs
  starved the queue (#4849).
- **Drop the main run:** it saves only 2%, and it would break the release gate.
- **Quarantine only and never retry** (Kubernetes): with today's flake rate, every flake would
  drop a queue entry.
- **Retry twice and disable through a bot** (PyTorch): it hides more, and needs a bot we don't
  have.

## Consequence

A red that a moved suite would have caught on the PR now lands in the queue. So each move to a new
stage is its own pull request, a week apart, and it is reverted if the queue drop rate over the
next 7 days is more than 5 points above the 15.3% baseline.
