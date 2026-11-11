# Script runs on Linux once a page's script is served as JavaScript

**2026-11-11, #6129, from the Linux scenario run of #6201.** Amends §10 of ADR 0122, and takes
back the amendment of 2026-11-10, *Script runs on macOS alone until the app's own isolation tests
are green on Linux*.

That amendment refused script on Linux because, in the app, a pinned artifact's child webview
never ran its page's script. The cause was not WebKitGTK's isolation: every response of an
artifact carries `X-Content-Type-Options: nosniff`, and its `app.js` was served as
`application/octet-stream`, which a browser under `nosniff` fetches and never runs. WebKit
enforces it on macOS and WebKitGTK alike. `serve::content_type` now names `.js` and `.mjs`
`text/javascript` (tested by `a_script_file_is_served_as_javascript_so_it_runs_under_nosniff`).

- **Linux is recorded green in the app.** The `ci` workflow's run 42975268037 at `15a5c691f`, on draft pull request #6201, its
  `scenario tests (ubuntu-24.04, part 1 of 2)` job 130372935850, ran the app's own isolation tests (reach, speak,
  walk, store, loop), the offers, the queries and the approval of a scripted page with the page's
  script running, and every one passed; M8 added 239 MB, within Linux's 264 MB budget:
  `tools/artifact-isolation-probe/results/linux-2026-11-11.txt`.
- **The gate is macOS and Linux**: `PROVEN = ["macos", "linux"]`, and script runs where
  `target_os` is either (`purlis_core::artifact::approval::SCRIPT_RUNS_HERE`). Windows stays
  refused, rendering script-free with purlis's sentence, until §3's tests pass on WebView2.
- **The scenario tests no longer assert a refusal.** They run on macOS and Linux alone, where
  script now runs, so each case that needs a page's script runs it on both, and the platform
  check and its refusal helper are gone.
- **The test that holds the gate to its records is unchanged in what it asks**: a probe run with
  all five green, and no newest record of the app's own tests failing. Linux's newest app record
  is now green.
