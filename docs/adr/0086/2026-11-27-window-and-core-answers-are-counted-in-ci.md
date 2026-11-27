# ADR 0086, amended: window and core answers are counted in CI, and a freeze and a read have evidence rows (2026-11-27)

**The targets ruled by the person** (2026-11-27, the felt-speed epic #7889, its standing rules);
**the rows written in drafting (#7890)**, for the person to accept by merging. Adds rows W1, W2
and A1 (CI absolute) and L16 and L17 (evidence only) to the budgets of record (`docs/spec.md`).
L11 and L12 do not change, and nothing else in this record moves.

**Why.** The window and core slow log of 2026-11-27 (about 2.3 h, 14 to 56 hot chats, #7889)
put 130 s into window stalls. About half was inside React renders, mostly side views drawn
again for changes they do not show; the trivial per-chat reads fell wholly inside stalls, each
pane mount making its own listeners and its own reads; and core answers such as `sandboxState`
started git and re-read the plane several times per ask. No row counted any of it, and a timing
row could not have held it in CI (§1). Each is a count, so each can be a CI absolute row that a
mutation turns red. The person's targets for what is felt (no freeze over 100 ms; window reads
p50 under 50 ms and p99 under 200 ms) are timings on the operator's machine, so they are
evidence rows beside L11 to L15.

**The rows.**

| # | What | Budget | Kind and job | Owner |
|---|---|---|---|---|
| W1 | window draws, at 50 chats and a set pane count, beats and clock held: renders of PlaneView, each Inbox part, ChangesView, Panels and ChatsSection, read by name through `renderedBy`, per tab switch, per settings change and per chat-state move | each component's recorded count, exactly; no count scales with the number of chats | CI absolute (`web`, the vitest shards, `web (part N of 4)`) | FS-G1a, #7894 |
| W2 | window listeners and asks, at W1's load: Tauri listeners per event name, and IPC asks per focus, per `plane-changed` kind, per chat start, per pane mount and per turn end | each recorded count, exactly; no count scales with the number of chats | CI absolute (`web`, the vitest shards, as W1) | FS-G1b, #7895 |
| A1 | what a core answer costs, per command of the `answer_class` table: process starts, `Plane::read` and `Locks::of`, counted on the blocking thread through the doors, per fixture (sandbox on; team off, committed) | a model-class command starts 0 processes; a git-class command stays within its recorded ceiling, cold and warm; `sandbox_state` is read at most 2 times per burst | CI absolute (`rust tests (part N of 2, app)`, the Rust test job that runs `app/src-tauri`'s `off_the_main_thread` tests) | FS-G2, #7892 |
| L16 | the longest frame per phase of `felt.bench.ts`, L11's and L12's phases and FS-G3's (plane-changed bursts, chat starts, a settings change) | no frame of 100 ms or longer | evidence only (`felt.bench.ts`, on the operator's machine, reported by FS-G3) | FS-G3, #7896 |
| L17 | window reads: each command's answer time from FS-M1a's per-command histogram, over `felt.bench.ts`'s phases | p50 < 50 ms and p99 < 200 ms, per command | evidence only (`felt.bench.ts`, on the operator's machine, reported by FS-G3) | FS-G3, #7896 |

**W1, W2 and A1 are exact counts, not timings.** They run in the jobs that already run the
tests they live in: W1 and W2 in the cost-test kit beside `planeViewDraws.cost.test.tsx`, A1
beside `costs.rs`. They need no job of their own and add no wall clock to a gate. Each is shown
red by one mutation before it is trusted (#7889's standing rules). A count read as a total would
let one view's extra draws hide behind another's fewer, so W1 reads each component by name.
A recorded count or ceiling moves only in a pull request that says why, as a recorded answer
does.

**A1 names what it does not count.** A command whose work runs through a raw `spawn_blocking`
is listed in the table as `Uncounted`, and a test fails once its site has moved to a door, so
its ceiling is recorded then and not left out by accident.

**L16 is stricter than L11 and does not replace it.** L11's frame limit (no frame of 200 ms or
more) stays the row of record. L16 reports, per phase, the frames between 100 and 200 ms that
L11 lets through, so the person's 100 ms target is seen before any row is moved to it. Where a
row of record is stricter than a target (L12's ≤ 100 ms, L15's 0), the row wins.

**L17 reads the core's own histogram.** FS-M1a keeps each command's answer time in memory and
writes it to `slow.jsonl` as one summary row an hour and at exit. FS-G3 reads it after it quits
the app. The window's own measure of an ask adds IPC and the window's thread, which L11 to L15
already hold. A bench run without that summary row has measured nothing and says so, as L15
does with a slow log that was never written.

**Kind.** L16 and L17 are *evidence only*, as the 2026-10-04 amendment says: a miss is written
into the bench's report and a warning, and the run does not fail for it. It fails only when the
run itself breaks. Until `bench` measures felt rows in CI (#931), they are read on the
operator's machine. Wall clock is never a gate here (#7889's standing rules).
