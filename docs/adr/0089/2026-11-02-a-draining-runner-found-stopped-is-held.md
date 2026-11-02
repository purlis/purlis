# Amended 2026-11-02: a draining runner found stopped is held with its home

*Decided in implementation, delegated by the person, to be confirmed.* This changes one line of
the 2026-10-27 amendment, item 5, *"the poll counts it as running no chat"* (D-2051-4). Nothing else of
it changes: a draining runner's poll still never wakes one.

1. **A stopped runner is not an empty one.** A stop ends a runner's chats, and what they left stays
   in its home. A look that took a stopped draining runner for one with no chats removed that home.
2. **As a drain begins, purlis notes when the runner's container last stopped** (`last_stop` in
   `runners.json`), as Docker says it. Only Docker's clock is read, so no two clocks are compared.
3. **A stop since then holds the runner, with its home**, whether it is still stopped or something
   started it again. It stays held: it is not woken to be asked, and no answer of its ends the
   hold, since a runner started again lists none of the chats its stop ended. A live link coming
   back is enough to start one (`ToARunner::of_kept_waking`), so a poll's answer of none is no
   proof after a stop.
4. **Bringing it back does not keep the chats.** Their programs ended with the stop. So a hold is
   the whole of what purlis does on its own. Picking a chat up from a held runner's home is left
   out of this.
5. **One that had already stopped when its drain began goes as before**: no chat ran on it then,
   and a draining runner starts none. Holding these too would keep an old home for each update of
   a pool left suspended.
6. **Only the person ends a hold**, with Remove runner on its row in Devices, from their window
   alone. Replace's dialog says a stop ends those chats and that purlis keeps the runner.
7. **A row from before `last_stop`** is held if it is found stopped, the safe side, and is asked as
   before while it runs.
