# Amended for #3043, a key whose page reads past its limit is held in words, 2026-10-26

1. **A key's read of the page has a limit under the window's.** A key of the person's that must
   read every frame's whole tree (a key into a second field of the page, or one that lands
   after a frame of the page moved on) waits for that read at most `key_reads_within` (20 s),
   counted from the key's arrival, so its answer comes before the window stops waiting for it
   (30 s). The tree around the first field (#2967) is read within the same limit.
2. **The read goes on past the key, and every key waits on the one read.** The read runs on a
   thread of its own, each frame's tree within `tree_reads_within` (120 s), not the 30 s of any
   other call: a large page's tree (17 to 46 s on macOS real Chrome for the page of #2295) is
   read whole even on a busy machine. A key the read outlives is held back, never sent, and
   answered `PageUnreadInTime`, in words that say the key was held and to type it again. Every
   key into the page, those typed at once included, waits on that one read, under way or come
   and not yet used, rather than asking the engine for another behind it; once a key used it,
   or it failed, the next key reads anew. It is shared only while the lease's epoch and the
   page's document number are what they were when it began, and no press of the mouse and no
   key that may move the focus (any key that does not edit, but a modifier's) reached the page
   since. *D-3043-1, D-3043-7, decided in implementation, to be confirmed.*
3. **A kept read is checked before a key uses it.** A read an earlier key began is used by a
   key only within `tree_read_kept_for` (10 s) after it came, and the field found in it only
   where its own document names it now as what has the focus, that document has the page's
   focus, and it holds no document of its own. Anything else holds the key back and has the
   page read anew, a field inside a shadow root (whose host the document names, #2972) and an
   element the engine cannot describe included; the key's own read, fresh, is not checked so. *D-3043-5, D-3043-6, decided
   in implementation, to be confirmed.*
4. **Never unknown.** A key whose read fails in time, its own limit or the read's, is answered
   `PageUnreadInTime`: the key was not sent, so its effect is known. `Unknown` stays for a key
   whose own sending outlived its limit. *D-3043-2, decided in implementation, to be confirmed.*
5. **The limits are the driver's.** `key_reads_within`, `tree_reads_within` and
   `tree_read_kept_for` are `drive::Limits` fields; making them configurable from Settings is
   #3301. *D-3043-3.*
