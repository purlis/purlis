# ADR 0086, amended: M8 is held per platform, Linux to its measure (2026-11-10)

**Decided in implementation (#5958, spec #5948)**, for the operator to accept in review. Amends
row M8 (*an open artifact tab has a row, M8*, 2026-11-09). Nothing else in this record moves.

**Why.** M8 was set at 192 MB before anything was measured, for macOS and Linux alike. Its first
measurements on Linux, where web content is counted by resident memory, all failed it:

| `ci` run | head | Linux, `scenario tests (ubuntu-24.04, part 1 of 2)` |
|---|---|---|
| 42760197611 | `52dfb08f9` | 224 MB (226 MB of web content, -2 MB of the app) |
| 42768595673 | `348813378` | 238 MB (241 MB of web content, -2 MB of the app) |
| 42776220936 | `5b265dbb9` | 223 MB (226 MB of web content, -3 MB of the app) |
| 42785758001 | `1dacdf0b1` | 230 MB (233 MB of web content, -2 MB of the app) |

One small page in a WebKitGTK web content process of its own costs that much there. The tab was
already script-free (the row measures one script-free page), so refusing script on Linux (ADR
0122, amended 2026-11-10) does not bring it under 192 MB. macOS, counted by `footprint`, keeps
192 MB: its scenario tests run in the merge queue, not on a pull request, so none of these runs
measured it.

**The row.**

| # | What | Budget | Kind and job | Owner |
|---|---|---|---|---|
| M8 | an open artifact tab: what it adds to the web content processes and the app once its page is drawn, one script-free page; footprint on macOS, resident on Linux | ≤ 192 MB on macOS, an initial value; ≤ 264 MB on Linux, its most measured (238 MB) with about a tenth more | CI absolute (`scenario tests`, macOS and Linux) | #5958 |

**How.** `app/e2e/artifactTabMemory.ts`'s `artifactTabBudgetKb` names the value for the platform
the scenario test runs on, and its tests hold both values. As before, every run writes both looks
and the verdict to `logs/artifact-tab.jsonl`, and the owner lowers either value from them.
