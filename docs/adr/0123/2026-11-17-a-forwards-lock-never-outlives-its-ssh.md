# Amended (2026-11-17): a forward's lock never outlives its ssh, and a shared forward is used only while its socket answers

Changes the words of §8 and of the amendment of 2026-11-12 on who holds a pool's forward. The
rest of this record stands.

## What was found

On the person's Mac, pool `docker` (a remote Docker over SSH) was unreachable for hours while its
machine and its runner were up. The window held the pool's `held.lock`; the ssh carrying its
forward had ended; the socket file it left was still in `engine-links/docker/`. The session host's
connector, another process, ran `docker info` against that socket every few minutes and was
refused each time.

Both halves of the rule let it happen:

- **The holder let go only when it next looked.** A process that opened the forward kept the lock
  and the socket file until its own next `hold` or `dropped` for that pool noticed the ssh had
  ended. The window may never look at the pool again.
- **A second process trusted the lock.** It took a forward another process held as alive while
  the lock was held and the socket file was there. Neither says anything forwards. And removing
  the socket by hand did not help: the second process waited for a socket the dead holder would
  never make again.

## The rule

1. **The holder lets go the moment its ssh ends.** Each forward has a watch, a thread of its own
   that asks every 200 ms whether its ssh has ended. When it has, the watch removes the socket and
   then releases the lock, in that order, so the process that takes the lock next never has its
   own socket removed. Its ssh, socket and lock are let go together, once, whether the watch, a
   drop or `let_go` gets there first.
2. **A forward another process holds is used only while its socket answers a connect.** An ssh
   still forwarding listens on the socket; the file a dead one left refuses the connect. This
   counts towards both `hold` and `dropped`, so PR #7116's reopening of a silent forward sees a
   dead shared forward as dropped too.
3. **A lock held over a socket nothing answers is waited out**, for up to the forward's own 30
   seconds. A holder built with rule 1 lets go within a moment, and the waiting process then
   takes the lock and opens a forward of its own. One that doesn't (an older purlis, a process
   stopped in a debugger) is refused with what to do: quit that process, or the window, and try
   again.

## Why both, and not one

- **Rule 1 alone** leaves a second process trusting whatever a holder built before this rule, or
  one stopped mid-way, left behind. The person's own window, running a build from before this
  fix, is such a holder.
- **Rule 2 alone** would find the dead socket but could not open a forward over a lock still
  held. It would only refuse more clearly. Rule 1 is what lets it take over.

## Alternatives not taken

- **Blocking on the ssh in the watch** (`Child::wait`) instead of asking every 200 ms. The watch
  would own the child, and ending the forward would then mean signalling a process id that may
  already have been reaped and reused. Asking `try_wait` under the same lock a drop takes has no
  such race, and the cost is one wake-up every 200 ms per pool held.
- **Reading the holder's ssh from its pid** in the second process. That is a process-table read
  on every `hold`, it differs by platform, and a pid can be reused. Connecting to the socket asks
  the one thing that matters: does something forward there.
- **Removing a stale socket and taking the lock by force.** A lock is either held or not. Taking
  it from a live process would put two forwards on one socket.
