# Amended 2026-11-03: a dispatch naming a pool this machine doesn't keep is refused at once

**Built by #4965**, on point 4 of the amendment of 2026-11-02 (the person's ruling that a chat's
dispatch that moves work needs their Ask). What follows is *decided in implementation, to be
confirmed* (the D-lines of the pull request that built it).

1. **A pool that isn't kept is no place to ask about.** A chat's dispatch that names a pool
   this machine doesn't keep, of either kind, starts nothing and is not held for the person: the
   chat is told, in words, that the pool isn't one this machine keeps, as it is told of a word
   purlis can't read. No start could take it, so an Approve would only fail.
2. **Asked before anything else of where it runs.** It is refused before the asks of a chat on
   a pool's runner are held (2026-10-30-07), and before a move is (2026-11-02, point 4), so it
   holds wherever the asking chat runs.
3. **A machine that keeps no pool, or whose pools can't be read, keeps none to name.** Its
   dispatch naming a pool is refused, saying why, rather than held for an Approve that a start
   would refuse.
4. **Only a refusal.** A pool this machine keeps still waits for the person's Approve as a move,
   and nothing here ever starts a dispatch or stands in for one.
