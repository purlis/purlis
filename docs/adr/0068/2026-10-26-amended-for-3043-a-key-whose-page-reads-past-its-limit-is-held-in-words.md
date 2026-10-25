# Amended for #3043, a key whose page reads past its limit is held in words, 2026-10-26

1. **A key's read of the page has a limit under the window's.** A key of the person's that must
   read every frame's whole tree (a key into a second field of the page, or one that lands
   after a frame of the page moved on) waits for that read at most `key_reads_within` (20 s),
   counted from the key's start, so its answer comes before the window stops waiting for it
   (30 s). The tree around the first field (#2967) is read within the same limit.
2. **The read goes on past the key.** The read runs on a thread of its own, each frame's tree
   within `tree_reads_within` (120 s), not the 30 s of any other call: a large page's tree (17
   to 46 s on macOS real Chrome for the page of #2295) is read whole even on a busy machine. A
   key the read outlives is held back, never sent, and answered `PageUnreadInTime`, in words
   that say the key was held and to type it again. The next key into the page waits on the same
   read, rather than asking the engine for another behind the one it is still making, where the
   lease's epoch and the page's document number are what they were when it began, and no press
   of the mouse and no key that may move the focus (any key that does not edit, but a
   modifier's) reached the page since. The element blanked is still checked against what its
   document says has the focus now (#2972). *D-3043-1, decided in implementation, to be
   confirmed.*
3. **Never unknown.** A key whose read fails in time, its own limit or the read's, is answered
   `PageUnreadInTime`: the key was not sent, so its effect is known. `Unknown` stays for a key
   whose own sending outlived its limit. *D-3043-2, decided in implementation, to be confirmed.*
