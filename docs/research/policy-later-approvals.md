# The person's approvals of `later:` rows

A row of `docs/research/policy-inventory-map.tsv` may take a `later:<issue>` target only when the
decision it records belongs to an issue of M80, "Chats as resources" (milestone 59), and the
person has approved the deferral by name (ADR 0125, amended 2026-11-27; #7746). This file is that
record. `every_inventory_row_is_mapped` reads it offline, never the live forge: a `later:` row
whose `approved:<name>` mark has no line here, naming the same row and issue, fails the check.

Each line names the row, its target, who approved it and the date. A new approval is a new line,
added by hand in a pull request.

| Row | Target | Approved by | Date |
|---|---|---|---|
| Dispatch #81 | `later:7755` | The person (the project owner) | 2026-11-27 |
| Dispatch #82a | `later:7756` | The person (the project owner) | 2026-11-27 |
| Dispatch #82b | `later:7755` | The person (the project owner) | 2026-11-27 |
| Dispatch #82c | `later:7773` | The person (the project owner) | 2026-11-27 |
| Dispatch #83b | `later:7756` | The person (the project owner) | 2026-11-27 |
| Browser #B69a | `later:7760` | The person (the project owner) | 2026-11-27 |
| Ports #P13 | `later:7760` | The person (the project owner) | 2026-11-27 |

The approval was given in one ruling on 2026-11-27: "The person approves deferring Dispatch #81,
#82a, #82b, #82c, #83b, Browser #B69a and Ports #P13 to M80, by name." The sub-rows (#82a, #82b,
#82c and #83b split from Dispatch #82 and #83, #B69a from Browser #B69) and their issues are the
plan review's split of rows with two targets. Until each issue lands, its row stays hard-coded as
it is today.
