# ADR 0086, amended: an open artifact tab has a row, M8 (2026-11-09)

**Decided in implementation (#5958)**, as ADR 0122 §10 asks (*"ADR 0086 gains a row for the
memory of an open artifact tab, with the job that measures it"*), for the operator to accept in
review. Adds row M8 to the budgets of record (`docs/spec.md`). Nothing else in this record moves.

**Why.** An artifact is drawn in a native webview of its own (ADR 0122 §3), so each open artifact
tab is a web content process the window did not have before. No row held what one costs.

**The row.**

| # | What | Budget | Kind and job | Owner |
|---|---|---|---|---|
| M8 | an open artifact tab: what it adds to the web content processes and the app once its page is drawn, one script-free page; footprint on macOS, resident on Linux | ≤ 192 MB, an initial value | CI absolute (`scenario tests`, macOS and Linux) | #5958 |

**How it is measured.** `app/e2e/specs/artifact-memory.e2e.ts` pins one small script-free page,
looks at the app with no artifact tab open, opens the tab, and looks again once the page's own
web content process has started and two looks a second apart agree within 2 MB. The row is the
web content processes' memory and the app's resident memory, after less before. Web content is
counted as M2 counts it: by `footprint` on macOS, where resident memory reads far low, and by
resident memory on Linux, so each system is its own measure. The verdict is
`app/e2e/artifactTabMemory.ts`, whose tests show it failing past the budget.

**A page drawn in no web content process of its own fails the row.** That process is what the
row measures, and it is why ADR 0122 chose the native webview. With none added, nothing was
measured, and a pass would say nothing.

**Why CI absolute, and in `scenario tests`.** It is a size, which §1 gives to CI absolute, and
`scenario tests` is the job that already drives the built app on macOS and Linux on every merge
and on `main`, so the row needs no job of its own. It does not scale with chats (§3): one tab
costs the same at any hot target.

**The value.** No measurement existed when it was set, since the sandbox it was written in can
list no processes. 192 MB is set above what one small page's web content process is expected to
take on either system, and below two of them, so a tab that opens a second webview fails. Every
run writes both looks and the verdict to `logs/artifact-tab.jsonl` and its output, so the first
green runs on `main` say what it costs, and the owner lowers the value from them, as §7 of
*Decided in drafting* allows.
