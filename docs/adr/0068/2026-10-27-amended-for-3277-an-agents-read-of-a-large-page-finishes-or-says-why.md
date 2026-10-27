# Amended for #3277, an agent's read of a large page finishes in its call's limit or says why, 2026-10-27

1. **An agent's read of the page has a limit under the window's.** A snapshot, find or wait of
   the agent's waits for its page's whole tree at most `page_reads_within` (60 s) after it
   settles: with the settling (10 s), under the 85 s the window waits for an act. A wait's reads
   are waited for until the wait's end, and at least as long as that. *D-3277-1, decided in
   implementation, to be confirmed.*
2. **The read goes on past the call, and the next call waits on it.** As for the person's keys
   (the amendment for #3043, item 2), the read runs on a thread of its own, each frame's tree
   within `tree_reads_within` (120 s). A call it outlives is answered `PageReadTooLong`, in words
   that say the page is too large to read in time, that the call did nothing, and to make it
   again in a moment. The next call of the agent's that reads the page waits on that read rather
   than asking the engine for another behind it, where the lease's epoch and the page's document
   number are what they were when it began, no call of the agent's that acts came between, and
   it came no more than `page_read_kept_for` (60 s) before. Once a call used it, or it failed,
   the next reads anew. An answer built from a read an earlier call began says how long ago it
   began. A call waiting on a read answers as soon as the person takes the browser over.
   *D-3277-2, D-3277-3, decided in implementation, to be confirmed.*
3. **Never unknown.** A read acts on nothing, so a read that fails in time, the call's limit or
   the read's own, is answered `PageReadTooLong`, never `Unknown`. A wait that read the page at
   least once and whose later read did not come in time answers that the text did not show.
   *D-3277-4, decided in implementation, to be confirmed.*
4. **The hand-back's read has the person's limit.** The hand-back, and a press of the mouse,
   read every page the person typed into within `key_reads_within` (20 s) in all, under the 30 s
   the window waits for them; a frame whose tree does not come in that time fails the read, so no
   frame is taken for empty for want of time. Where a page's read does not come, the groups of
   boxes on it (the amendment for #2144) are found in the read the person's last key into that
   page was blanked from, kept until the hand-back, joined from what each box holds now, read
   from the box itself; the host's log says the page could not be read and that its groups were
   rebuilt so, or, with no such read, that none was. *D-3277-5, D-3277-6, decided in
   implementation, to be confirmed.*
