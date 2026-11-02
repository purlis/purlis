# Amended 2026-11-02: a draining runner found stopped is held with its home

*Decided in implementation, delegated by the person, to be confirmed.* This changes one line of
the 2026-10-27 amendment, item 5, *"the poll counts it as running no chat"* (D-2051-4). Nothing else of
it changes: a draining runner's poll still never wakes one.

1. **A stopped runner is not an empty one.** A stop ends a runner's chats, and what they left stays
   in its home. A look that took a stopped draining runner for one with no chats removed that home.
2. **As a drain begins, purlis notes when the runner's container last stopped** (`last_stop` in
   `runners.json`), read before the new runner is made. Only Docker's clock is read, so no two
   clocks are compared.
3. **A stop since then holds the runner, with its home**, whether it is still stopped or something
   started it again, and the row says so for good (`held`). It is not woken to be asked, and no
   answer of its ends the hold, since a runner started again lists none of the chats its stop
   ended. A live link coming back is enough to start one (`ToARunner::of_kept_waking`), so a
   poll's answer of none is no proof after a stop.
4. **Bringing it back does not keep the chats.** Their programs ended with the stop. So a hold is
   the whole of what purlis does on its own. Picking a chat up from a held runner's home is #4632.
5. **Replace on a runner already stopped holds it**: the person pressed it, and what a stop before
   that ended is theirs to let go of. **An update on one goes as before**, with what chats left in
   its home: no chat is running on it then, but one a suspend ended may have left its conversation
   there. Holding these too would keep an old home for each update of a pool left suspended, on
   no one's press. This is the open part, #4633, and waits for the person's word.
6. **Only the person's press ends a hold**: Remove runner on its row in Devices, from their window
   alone, or removing its pool. A runner whose container was removed outside purlis stays held
   with what is left of it.
7. **A row from before `last_stop`** is held if a look finds it stopped, the safe side, and is
   asked as before while it runs.
