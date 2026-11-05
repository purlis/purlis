# Amended 2026-11-05: a fresh runner's check has a stated time limit

**Built by #5312**, following #5112's amendment of the same day, whose point 4 left a check's cost
to the image. What follows is *decided in implementation, to be confirmed* (the D-lines of the pull
request that built it).

1. **One check looks through a runner's files for at most 5 minutes**, on the runner's own clock
   (`held::LOOK_WITHIN`). The look through its projects, the first look at where its user may write
   and every walk after them share those 5 minutes. Past them nothing more is looked at, and the
   check says so. One git call under way finishes first, so the window's clock is what bounds a
   repository that holds git up.
2. **The window waits that long plus its wait for any answer**, on this machine's clock, for the
   proof's list only. A start's question of whether a runner holds its own code keeps the wait for
   any answer, since it may run under a lock the window's commands wait on. The proof holds the
   one-at-a-time lock on fetches meanwhile, so another chat's fetch-back can wait that long too. No
   window command waits on that lock.
3. **A check that runs out keeps the runner**, as anything not looked through does. Its sentence
   comes first among what the runner says, since the Notice names only the first few, though a
   clone that did not come back is said before it. It says why, and what helps: Fetch again, a
   smaller image for the pool's next runners, and Discard once what it holds isn't needed.
4. **A first look that ran out is not kept**, so the next check starts it again. One that finished
   is kept, even where the rest of its check ran out, so the next check has only the rest to do.
5. **Why 5 minutes:** on a CI runner, with its caches dropped, 1,000,000 files in 5,000 folders took
   14 to 18 seconds for a first look and 20 to 29 for a check, and about 5 for either with them warm.
   That was a debug build on an ordinary disk, and no real cluster's disk was timed, so the limit
   leaves room for one several times slower.
6. **A Discard pressed during a check is kept**, and the runner goes when that check ends. A sweep
   starts a check about once a minute, so on such a runner a press would otherwise nearly always
   be dropped. Fetch again during a check does nothing more: the check under way says how it went.
7. **Rejected:** a new pool limit, since an image too large for this is better made smaller than
   waited on, and a person has no way to know the right number; only the window's wait, since the
   look would run on after the window gave up and each sweep, once a minute, would start another
   beside it; raising the wait for any answer, which every other call shares.
