# ADR 0086, amended: five rows for what the operator feels, and L9 under load (2026-11-02)

**The rows ruled by the operator** (2026-11-02, the felt-speed grill, #4724 item 11); **their
limits decided in implementation (#4744)**, for the operator to accept in review. Adds rows L11
to L15 to the budgets of record (`docs/spec.md`) and changes L9's definition. Nothing else in
this record moves.

**Why.** The operator's five symptoms (#4724) all came with many agents busy, and no row was
measured that way: L2 has never been measured with many busy chats, L9 was measured with one
idle chat per project, and nothing covered frame time while agents stream, the first keystroke
after a switch, the new-chat dialog, or a command answered late. Each new row is measured under
the load it was felt under, with the fake harness, by one bench spec,
`app/e2e/bench/felt.bench.ts`, read by `app/e2e/feltRows.ts`.

**The rows.**

| # | What | Limit | Where the limit comes from |
|---|---|---|---|
| L11 | frames while 20 agents stream behind an idle chat, 10 s | p95 ≤ 33 ms, no frame of 200 ms or more | L4's 30 fps floor; #4725's stall |
| L12 | a chat switch at 50 busy chats, p95 of 20 | ≤ 100 ms | L2, the tab switch |
| L13 | 3 keystrokes the moment each L12 switch has painted, p95 of 60 | ≤ 50 ms | L1, keystroke to screen ("the same budget as any other keystroke", #4724 story 3) |
| L14 | the new-chat dialog, 200 personas, L12's load still on, p95 of 20 | ≤ 100 ms | L2: "opens at once" (#4724 item 6) |
| L15 | answers of 1 s or longer in `slow.jsonl` during that run | 0 | #4725's threshold for a late answer |

**L9, revisited.** L9 is now measured with 20 agents streaming in the first of the ten projects
the whole time (`app/e2e/bench/projects.bench.ts`). Its limit, statistic and press-to-paint
span do not change. An idle window is not the one the operator switches in, so a met L9 said
nothing about the symptom.

**Kind.** Each is release absolute, on the operator's machine, like L1 to L4. A miss is
recorded as `MISSED` beside the numbers (`tools/window-verdicts.mjs`) and is a bug filed before
the release notes, not a blocked release. CI relative in `bench` waits for #931, as L2's does.

**L15 reads the slow log, not the late-answer list.** The window's 20 s line
(`src/answerWatch.ts`) says only what is very late. The slow log writes down every answer of
1 s or more, which is what the operator felt and what the row is about. A stall in that log is
reported beside L15 and not judged by it: L11 holds the frames.
