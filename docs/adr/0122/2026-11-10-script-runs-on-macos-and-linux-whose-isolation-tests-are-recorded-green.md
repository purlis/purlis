# Script runs on macOS and Linux, whose isolation tests are recorded green

**2026-11-10, spec #5948, from the adversarial review of #5978.** Amends §3 and §10 of ADR 0122.

The record refuses an artifact's script on a platform until §3's five tests are green there, but
the gate in the code was "not Windows" (`cfg!(not(windows))`): it let script run on Linux with no
recorded result, and on any other unix with no run at all.

- **Linux is recorded green.** The `artifact-isolation` workflow's run 42714081704, at
  `54ceb120d` on 2026-11-09, passed all five tests on ubuntu-24.04 (WebKitGTK under xvfb) for the
  chosen rendering, and each of the four unguarded controls failed for its own reason, as on
  macOS: `tools/artifact-isolation-probe/results/linux-2026-11-09.txt`.
- **Windows is recorded red, so script stays refused there.** The same run failed test 4 on
  WebView2: a script-free page's `<meta refresh>` reached the canary and a sibling artifact's
  file. Its unguarded `reach` control also passed, so that control does not test its guard on
  Windows: `tools/artifact-isolation-probe/results/windows-2026-11-09.txt`.
- **The gate names the proven platforms and nothing else**: `PROVEN = ["macos", "linux"]`, and
  script runs only where `target_os` is one of them
  (`purlis_core::artifact::approval::SCRIPT_RUNS_HERE`). Every other platform, Windows and every
  other unix, renders script-free and asks nothing. A test holds the gate to the records: each
  proven platform has a results file with all five green, Windows has none, and the gate
  matches the list. Adding a platform is a recorded green run first, then this list.
