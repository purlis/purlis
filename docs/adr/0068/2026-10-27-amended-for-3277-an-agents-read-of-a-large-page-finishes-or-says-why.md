# Amended for #3277, an agent's read of a large page finishes in its call's limit or says why, 2026-10-27

1. **An agent's read of the page has a limit under the window's.** A snapshot, find or wait of
   the agent's waits for its page's whole tree at most `page_reads_within` (60 s) after it
   settles: with the settling (10 s), under the 85 s the window waits for an act. A wait's reads
   are waited for until the wait's end, and at least as long as that. *D-3277-1, decided in
   implementation, to be confirmed.*
2. **The read goes on past the call, and the next call waits on it.** As for the person's keys
   (the amendment for #3043, item 2), the read runs on a thread of its own, each frame's tree
   within `tree_reads_within` (120 s), the engine's answers held to `most_tree_read_bytes` in
   all. The agent's calls and the person's keys each keep their own read; a read begun under one
   lease is never used under another, and a take-over, a hand-back and a login keep none past
   them. A call the read outlives is answered `PageReadTooLong`, in words that say the page is
   too large to read in time, that the call did nothing, and to make it again in a moment. The
   next call of the agent's that reads the page waits on that read rather than asking the engine
   for another behind it, where the lease's epoch and the page's document number (which any of
   its frames loading another document moves) are what they were when it began, and no call of
   the agent's that acts came between: an act leaves the read for no later call once no call
   that only reads still runs. A read that came is used no more than `page_read_kept_for` (60 s)
   after it came; one still under way, no more than `page_reads_within` and
   `page_read_kept_for` after it began. A wait uses only a read that began once the wait was
   made. Once a call used a read, or it failed, the next reads anew. An answer built from a read
   an earlier call began says how long ago it began. A call waiting on a read answers as soon as
   the person takes the browser over. *D-3277-2, D-3277-3, decided in implementation, to be
   confirmed.*
3. **One whole-tree read at a time.** Each of the two keeps asks the engine for one read of a
   page's whole tree at a time: where a read no one will use any more (an act, or a key that may
   move the focus, left it) is still under way, the next read is asked for only once it has
   ended, and a call or key whose limit passes before then is answered as one the read outlived.
   *D-3277-8, decided in implementation, to be confirmed.*
4. **Never unknown.** A read acts on nothing, so a read that does not come in the call's limit
   is answered `PageReadTooLong`, and one that did not finish within its own limit
   `PageReadPastItsLimit`, whose words say the read ended and do not ask for the call again in a
   moment; never `Unknown`. A wait that read the page at least once and whose later read did not
   come answers that the text did not show. *D-3277-4, decided in implementation, to be
   confirmed.*
5. **The hand-back's reads have the person's limit.** The hand-back, and a press of the mouse,
   read every field the person typed into, then every page they typed into, within
   `key_reads_within` (20 s) in all, every call of those reads (the frames' owners included) made
   by then; a frame whose tree does not come in that time fails the page's read, so no frame is
   taken for empty for want of time. A field whose read does not come keeps what it was last
   read to hold. Where a page's read does not come, the groups of boxes on it (the amendment for
   #2144) are found in the read the last field the person typed into on that page was blanked
   from, kept until the hand-back, joined from what each box holds now, read from the box itself;
   the host's log says the page could not be read and that its groups were rebuilt so, or, with
   no such read, that none was. What else the hand-back does (letting go of a button still held,
   emptying a login the person cut) keeps its own limits. *D-3277-5, D-3277-6, decided in
   implementation, to be confirmed.*
6. **The limits are the driver's.** `page_reads_within`, `page_read_kept_for` and
   `most_tree_read_bytes` are `drive::Limits` fields; making them configurable from Settings is
   #3301.
