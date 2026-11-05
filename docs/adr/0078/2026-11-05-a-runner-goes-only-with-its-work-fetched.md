# Amended 2026-11-05: a runner goes only with its work fetched

**Built by #5393.** What follows is *decided in implementation, to be confirmed* (the D-lines of
the pull request that built it). It changes item 2 of the
[amendment on a fresh runner's check](2026-11-05-a-fresh-runners-check-has-a-stated-time-limit.md),
whose proof held the lock on fetches.

1. **A fresh runner's proof runs outside the one-at-a-time lock on fetches.** It writes nothing
   here, so it holds up no other chat's fetch. Its fetch stays under the lock. A runner whose fetch
   failed is not looked through, as it stays whatever that would find.
2. **The sweep asks a kept fresh runner less often once its end took long**: ten times what the end
   took, from a minute up to an hour. That is measured, not read from what the runner said, so it
   needs no sentence to be recognised. Fetch again and a chat's end don't wait for it. A window's
   restart forgets it, so that costs one more check.
3. **A look that took longer than the wait for any answer is taken again**, and the runner goes
   only on a second that passes within that wait. Past it, the runner stays and says what helps.
4. **A draining runner goes only with its work fetched.** A look at it finds no chat left, then it
   stays while any project this machine remembers owes a fetch from its pool (`branchesToFetch`) or
   keeps a chat that may be on it (`chatsOnPools`). The look reads the record, so it covers a project
   whose window is closed. If the record can't be read, the runner stays. A runner already unpaired
   takes no link, and goes as before.
5. **A chat's record names the runner it started on** (`runner` in `chatsOnPools`, in `machine.json`,
   Machine tier, device-bound), so a chat on the pool's next runner holds no draining one back. A
   record with no name counts for every runner of its pool: one from before this, or one a restart
   found. A record kept again on its pool keeps the name it had.
6. **A fetch settles what is owed only once it has asked every runner of the pool that runs.** A
   draining runner no host reaches is asked over `<pool>/<runner>`, and one a link reaches already
   gets no second link. What can't be asked is a failure, so it stays owed. A runner's word that it
   holds no such repository is nothing to fetch, or a runner a workspace never ran on could never go.
7. **A stopped draining runner is not woken to be asked**, as before (#2051). One stopped during its
   drain is held for the person, and one stopped before it goes by whether its home holds a
   conversation (#4633).
8. **The way out** of a runner held by a fetch that can't finish is removing its pool, which forgets
   what the pool's records owe.
9. **Rejected:** a machine-wide count of fetches under way in the window, which says nothing across a
   restart or for a project not open; fetching from inside the look, which can't reach a project not
   open; proving each draining runner's work home as a fresh runner's is, which needs each project's
   checkout on hand and asks far more of a runner shared by many projects.
