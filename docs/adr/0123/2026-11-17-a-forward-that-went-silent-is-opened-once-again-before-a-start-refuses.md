# Amended 2026-11-17: a forward that went silent is opened once again before a start refuses

**Found on a live remote Docker pool** whose machine restarted. §8 opens a pool's forward again
where it dropped, and judges it dropped when its `ssh` has ended or its socket is gone. After the
far machine restarted, `ssh` had not noticed yet (it notices on the next write or keep-alive), so
the forward looked alive, the engine's check through it failed (`docker info`: cannot connect to
the Docker daemon at the forward's socket), and the start was refused. A later Try again worked,
because by then `ssh` had ended. *Decided in implementation, to be confirmed* (D-7091-4):

1. **A forward held from before whose engine does not answer is opened once again once its
   `ssh` has ended**, and the engine checked through the new one, before anything is refused
   (`sshroute::pool::checked_or_opened_again`). The check's own write is what tells `ssh` that
   its far end went away, so it is given two seconds to end (`NOTICED_WITHIN`). One whose `ssh`
   runs on is kept: the engine is only slow, and the forward carries other links' streams, which
   letting it go would cut. Once only: a forward just opened that is silent is looked at to say
   why, as before, and an engine that answers as another than the pinned one is never tried
   again.
2. Opening it again goes through the same pinned `known_hosts` as any opening, so nothing about
   the host key's check changes.
3. A forward held by another process (`purlis pool …` beside the window) is that process's to
   open again; this changes nothing there.
