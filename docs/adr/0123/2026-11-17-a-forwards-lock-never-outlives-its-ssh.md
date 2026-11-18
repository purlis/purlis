# Amended (2026-11-17): a forward's lock never outlives its ssh, and a shared forward is used only while its socket answers

Changes the words of §8 and of the amendment of 2026-11-12 on who holds a pool's forward. The
rest of this record stands, D-6257-1 among it, for the reasons below.

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

## First: should this be purlis's own mechanism at all?

The person's ruling was to fix it idiomatically, so the standard ways were weighed before the
rule was mended.

- **Docker's own `ssh://` transport** (`DOCKER_HOST=ssh://…`, which runs
  `docker system dial-stdio` on the other machine for each connection). It has no long-lived
  forward and so nothing to go stale. It still fails D-6257-1's reasons: it runs `ssh` with the
  person's own settings and takes no pinned `known_hosts`; it connects anew for each `docker`
  command, and a link runs several; and it needs a `docker` program on the other machine, which a
  Podman pool doesn't have (Podman reaches its socket with its own client, amendment of
  2026-11-12). Not taken.
- **OpenSSH's own multiplexing** (`ControlMaster`, `ControlPath`, `ControlPersist`). ssh owns the
  master's socket and `ssh -O check` tests it. It does not remove what broke here:
  - `docker` needs a local socket, not a command to run. OpenSSH's `-W` forwards standard input
    and output to a `host:port` only (ssh(1)), not to the engine's socket. So the master would
    still have to carry the same `-L` listener, and something would still have to decide which
    process opens it and when it is dead. That is the lock this record keeps, moved into ssh.
  - `ControlPersist` puts the master in the background, apart from the process that opened it.
    The amendment of 2026-11-12 made every forward end with the process holding it, by a pipe, so
    the remote parity job's orphan check finds none. A persisted master is such an orphan by
    design.
  - Two processes racing on `ControlMaster=auto` can each end up with a connection of its own.
    With `StreamLocalBindUnlink=yes`, the second unlinks the first's listener.
  - §2 already passes `ControlMaster=no` and `ControlPath=none`: no master shared with another
    ssh, and none opened.

  Not taken.
- **The `-L` forward with the usual rule for a shared resource**: its owner releases it when its
  child exits, and a user counts it as alive only while it answers. This keeps D-6257-1 whole and
  mends only the rule that broke. **Taken**, as below.

## The rule

1. **The holder lets go the moment its ssh ends.** Each forward has a watch, a thread of its own
   that asks every 200 ms whether its ssh has ended. When it has, the watch removes the socket and
   then releases the lock, in that order, so the process that takes the lock next never has its
   own socket removed. A forward's ssh, socket and lock are let go together, once, whether the
   watch, a drop or `let_go` gets there first. A forward that has let go no longer names its
   ssh's process id, which may be another process's by then.
2. **A forward another process holds is used only while its socket answers a connect.** An ssh
   still forwarding listens on the socket; the file a dead one left refuses the connect. This
   counts towards both `hold` and `dropped` (a shared forward whose socket doesn't answer has
   dropped). Each ask is a connection ssh carries to the engine as a channel of its own, closed at
   once. A forward this process opens counts as open once its socket answers, not once its file
   is there.
3. **A lock held over a socket nothing answers is waited out for 2 seconds.** A holder built with
   rule 1 lets go within 200 ms. The waiting process then takes the lock and opens a forward of
   its own. One that doesn't let go (an older purlis, or a process stopped in a debugger) is
   refused with what to do: quit that process, or the window, and try again. The wait is short
   because it holds the process's table of forwards. A holder with no socket yet is still opening
   it, and is waited for up to the forward's own 30 seconds, as before.

## Why both, and not one

- **Rule 1 alone** leaves a second process trusting whatever a holder built before this rule, or
  one stopped part way through, left behind. The person's own window, running a build from before
  this fix, is such a holder.
- **Rule 2 alone** would find the dead socket but could not open a forward over a lock still
  held. It would only refuse more clearly. Rule 1 is what lets it take over.

## Alternatives not taken in the rule

- **Blocking on the ssh in the watch** (`Child::wait`) instead of asking every 200 ms. The watch
  would own the child, and ending the forward would then mean signalling a process id that may
  already have been reaped and reused. Asking `try_wait` under the same lock a drop takes has no
  such race. The cost is one wake-up every 200 ms per pool held.
- **Reading the holder's ssh from its pid** in the second process. That is a process-table read
  on every `hold`, it differs by platform, and a pid can be reused. Connecting to the socket asks
  the one thing that matters: does something forward there.
- **Taking a stale lock by force.** A lock is either held or not. Taking it from a live process
  would put two forwards on one socket.
