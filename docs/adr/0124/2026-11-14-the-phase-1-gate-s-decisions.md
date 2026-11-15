# Amendment, 2026-11-14: what the phase-1 gate decided (Q268, Q269, Q279, Q280)

Phase 1's reviews and its gate changed four things this record says. The gate checked the record
against what shipped, and this amendment brings it in line. The orchestrator decided each of them,
for the person's review.

**A merge always takes two presses** (Q268, Q269, from T1-39's and T1-40's reviews). Merges,
decision 2, said a merge takes one press when the head is the SHA the person last viewed in purlis,
or the current head has an approving review. It never takes one press:
- The row still says whether the head was viewed or approved, and otherwise "2 commits since you
  looked". Neither fact saves a press.
- A merge hold that arrives without its risk class asks once more, with a sentence of purlis's own.
- The core enforces the second press too. `merge_held_request` merges only after
  `reread_held_merge` has asked once more and the facts have not changed since.

View changes still marks the head as viewed (Q168), for the row to say so.

**A handoff inherits the attendance of the chat that asked for it** (Q279, R-358, from the gate's
code review). Attendance moves one way (Q105), and a new chat must not be a way round that:
- A handoff an unattended chat asks for starts unattended, whatever workspace it lands in, and
  stays unattended.
- So does a handoff whose asking chat purlis holds no fact for (fail closed, as A4 reads a missing
  fact).
- A handoff from the window, or from an attended chat, starts attended.

**The finish row shows every block the Blocked marker counts** (Q280, narrowing Q263, from the
gate's drift review). That includes harness-check, Can't-tell and unverified blocks, so the marker
and the finish row never disagree. What resolves a block stays Q263's rule.

**An Allow resets only the blocks it covers** (Q280). In the loop table, the block loop's "Reset by"
said "or an Allow". It is now an Allow of a sandbox ask, which resets the aims of the sandbox blocks
it covers. No other Allow, the harness's own prompt included, resets any. One function decides it,
for the loop and for the finish row alike (`looping::allow_resolves`).
