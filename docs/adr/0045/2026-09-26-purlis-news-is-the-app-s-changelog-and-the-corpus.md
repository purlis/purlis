# Amendment, 2026-09-26: `purlis news` is the app's changelog, and the corpus is gone

Issue #352. `purlis news` with no flags told every plane *"no baseline recorded, so there is no
range to report"*. The range view needed the update baseline the Python charter's `purlis
update` stamped, and nothing in this app writes one. The operator ruled: **retire the range
view; `purlis news` shows the app's own CHANGELOG.md sections.**

- `purlis news` prints every section of the `CHANGELOG.md` compiled into the binary, newest
  first, with `[Unreleased]` only when it has something in it. `purlis news --for <version>`
  prints one section, the same notes the release page and About purlis show. All three read
  the file through the `changelog` crate.
- `--since`, `--until` and `--pending` are retired. Each is still accepted and refused by name,
  exit 1, with what to run instead, because an agent reading an older skill will type them.
  `--pending` had nothing left to probe: every `check:` in the corpus named a command this
  binary does not have, so every entry reported *unchecked*.
- Section 4 is void. `crates/purlis-core/news/` (339 files), its `build.rs` inclusion,
  `news::HISTORY_REPO`, `news::history_ends`, the probe machinery (`PROBEABLE`,
  `CHARTER_NEWS_PROBE`) and the release-body renderer are deleted. `adopt::PYTHON_LINE_LAST`
  stays a constant, as section 3 argued, and no test ties it to a corpus any more.
- `purlis update` reads no baseline. It says it installs nothing, names the channel, and
  points at `purlis news`.
- The pin dialog's news list is gone. It was always empty, as section 4 said it would be.
- The recorded rows for `news` changed with it (ADR 0046). `news-for-a-version-that-quotes-a-
  headline-is-refused-before-it-publishes`, `news-range-lists-the-same-entries-in-the-same-order`
  and `news-with-no-baseline-reports-no-range` are removed. `news-for-a-version-nothing-shipped`
  is renamed `news-for-a-version-the-changelog-does-not-have`. The new rows are
  `news-for-a-released-version-prints-its-changelog-section`,
  `news-prints-the-changelog-newest-first-with-no-baseline`,
  `news-range-flags-are-retired-and-say-what-to-run` and
  `news-pending-is-retired-and-says-what-to-run`.
