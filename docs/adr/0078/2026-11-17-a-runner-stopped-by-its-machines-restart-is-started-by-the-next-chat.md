# Amended 2026-11-17: a runner stopped by its machine's restart is started by the next chat

**Found on a live remote Docker pool** (Ubuntu 24.04 over SSH, its kernel updated and the machine
restarted). The runner's container came back `Exited (0)`, and the person started it by hand.
What follows was decided in implementation and is marked *decided in implementation, to be
confirmed*.

1. **A runner's container has no restart policy, and keeps none.** Docker's `unless-stopped` or
   `always` would bring it back after a restart, but would also start, at once, a runner that
   suspended itself after its pool's time without a link (#2051): its own exit is what
   suspending is, and Docker cannot tell it from a crash. It would also start every runner on
   every boot of its machine, linked or not. `on-failure` does not help: a runner stopped by a
   shutdown exits 0. *Decided in implementation, to be confirmed* (D-7091-1).
2. **The next chat starts it, as it starts a suspended one.** A chat's link to its runner was
   already able to (`Docker::resumed`, through `ToARunner::of_kept_waking`), and the window
   already shows a stopped runner as one the next chat starts. On a pool on another machine it
   did not, because the check before the link asks the engine through the pool's SSH forward
   first, and after the restart that forward was dead though it looked alive: the check refused,
   and the wake never ran. The forward is now opened once again before the check refuses (ADR
   0123, amended this day), so the wake runs. *Decided in implementation, to be confirmed*
   (D-7091-2).
3. **The desktop's first wait for a link outlasts the link's own deadlines** (`FIRST_LINK_WITHIN`
   in `runnerlinks`): the check before the link is held to `CHECK_WITHIN`, and the hello,
   handshake and session proof each have the protocol's own deadline. Before, the desktop gave up
   after 20 seconds, part way through a handshake its slow check had pushed late, and said only
   that the runner did not answer in time; the runner, its stream closed under it, logged a
   handshake that failed against another host. Now the link says why it is down, with where to
   look (Pool setup), and a runner whose dialer goes away mid-handshake says that it went away
   (`LinkRefused::WentAway`), never that it is another host. *Decided in implementation, to be
   confirmed* (D-7091-3).
