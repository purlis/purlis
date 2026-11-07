# Amended 2026-11-06: the Check reporter, and protocol 6 (ADR 0117, #5410)

The vocabulary gains one word:

| Word | What purlis does | Its shape, under `contributes` | Change |
|---|---|---|---|
| `check-reporter` | hands its program a check's output and exit status, and records the SARIF 2.1.0 it answers on stdout as the branch's check results (ADR 0117) | none | #5410 |

**It has no shape.** Which tool it converts is the extension's own business: the operator approves
that it is asked, and a report names the reporter by the extension's id. A manifest that asks for it
must name a program (`runs`), or it is refused.

**Protocol 6 holds its one request.** `{"charter": 6, "extension": "<id>", "check": {"output":
"<text>", "status": <n>}}`, where `output` is what the check printed, read as UTF-8 with anything
else replaced, at most 8 MiB, and `status` its exit status. The answer is not a line of JSON: it is
what the program prints on stdout, which must be SARIF 2.1.0, and it must exit 0. A program that
exits with another status gave no answer, and what it printed on stderr is quoted. Its stdout is
bounded like a command's (512 KiB), and it has the normal deadline. `check-reporter` needs protocol
6, so a manifest naming an earlier `version` is refused: protocol 5 is the router's (#4680, ADR
0101 as amended 2026-11-07), which the Check reporter is stacked on. Its messages are recorded in
`docs/extension-api.md`.

It is proven end to end by its own probe, `crates/check-probe`, as the studio capabilities are by
`studio-probe`: the extension probe speaks protocol 2, and moving it to 6 would ask every capability
it proves in a protocol it was not proven in.
