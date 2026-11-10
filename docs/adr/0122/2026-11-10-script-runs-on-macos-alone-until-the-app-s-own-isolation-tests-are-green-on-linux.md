# Script runs on macOS alone until the app's own isolation tests are green on Linux

**2026-11-10, spec #5948, from the Linux scenario runs of #5978.** Amends §10 of ADR 0122, and
takes back the Linux half of the amendment of the same day, *Script runs on macOS and Linux,
whose isolation tests are recorded green*.

§10 refuses an artifact's script on a platform until it is proven isolated there. The probe
(`tools/artifact-isolation-probe`) proved a bare webview isolated on Linux, but in the app a
pinned artifact's child webview never runs its page's script at all, so the app's own isolation
tests (`app/e2e/specs/artifact-isolation.e2e.ts`) have never heard a Linux page, and nothing
proves the app isolated there.

- **Linux is recorded red in the app.** The `ci` workflow's Linux scenario job failed the same
  way twice: run 42776220936 at `5b265dbb9`, with the artifact webview no longer incognito, and
  run 42785758001 at `1dacdf0b1`, with the page also navigated to its approval's own origin
  rather than reloaded. Isolation tests 1, 3, 5 and 6 found their page silent, the offers group
  never appeared, and the queries spec heard no answer:
  `tools/artifact-isolation-probe/results/linux-2026-11-10.txt`, beside the probe's green
  `linux-2026-11-09.txt`.
- **The gate is macOS alone**: `PROVEN = ["macos"]`, and script runs only where `target_os` is
  macOS (`purlis_core::artifact::approval::SCRIPT_RUNS_HERE`). On Linux, as on Windows, a
  scripted page renders script-free, asks nothing, and purlis's bar says so in its sentence
  (*"Shown without its script: scripted artifacts do not run on this system yet"*); an approval
  is refused.
- **A platform is proven by both records.** The test that holds the gate to the results files
  now asks two things of each proven platform: a probe run with all five green, and no newest
  record of the app's own isolation tests failing there. Linux is taken back into `PROVEN` once
  a record of the app's tests on Linux is green and newer than `linux-2026-11-10.txt`.
- **The scenario tests assert the refusal on Linux.** Each case that needs a page's script
  (isolation 1, 2, 3, 5 and 6, the offers, the queries' answers, the approval of a scripted
  page) asserts on Linux, by a platform check and never by a skip, that the page is shown
  without its script and offers no approval. The cases that need no script run everywhere.
- **A test build still runs script on Linux.** Only a build with `test-support`, which no
  shipped build and no scenario-test build has, judges every unix as proven, so the approval
  record and the serving of an approved page stay tested by `cargo test` on Linux, where CI
  runs it.
- **macOS's app tests are not yet seen in CI.** Its scenario tests run in the merge queue, not
  on a pull request, so macOS is proven by the probe, run by hand on 2026-11-09, until the
  merge queue's run records the app's own.
