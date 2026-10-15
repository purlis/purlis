# Ruled (V77, 2026-10-03, the HP-2 review)

5. **A level-3 agent leads its own process group, not its own session, and the host never has
   a controlling terminal.** `process_group(0)` stays; no new `unsafe`. The app and `purlisd`
   leave their terminal as they start, with a test that a host started in a terminal has none
   afterwards, and none after it opens a terminal pair. §1 is amended to say so.
