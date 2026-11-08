# Amended 2026-11-08: a chat with no record is taken back once per project

**Ruled by the person, 2026-11-08**, amending point 1 of the amendment of 2026-11-02, *a chat from
before the record is kept once taken back* (D-4683-1).

1. **A chat with no record is taken back on its pool's word at one restart alone.** The first time
   a window of a project finishes putting its chats back (`close_what_was_not_reattached`), this
   machine's store marks that project's entry (`poolChatsRecorded`, `docs/plane-format.md`). Until
   then, a chat with no record that the pool its workspace runs its chats on says it runs is taken
   back and kept as D-4683-1 point 1 says. From then on, every chat the window starts on a pool has
   a record before it starts, so a chat with none is never asked of a pool: it starts afresh here,
   and nothing a pool says of that number is taken.
2. **Why.** A chat with no record is either one from before records or one that never ran on a
   pool: one that ran on this machine and ended, or that its host lost. Its number is the window's,
   and with no mark a runner of the pool, now or draining, could answer for it. After the first
   restart the first kind is gone, so only the second is left, and no pool's word is needed for it.
3. **What it costs.** A chat from before records whose runner did not answer at that first restart
   is not taken back by a later one. It ends on that runner as any chat no window takes back.
