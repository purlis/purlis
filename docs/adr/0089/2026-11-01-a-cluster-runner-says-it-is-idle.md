# Amended 2026-11-01: a cluster runner says it is idle

**Built by #3980** on spec #2041's idle stop and §9 as amended on 2026-10-27. *Decided in
implementation* (D-3980-1 to D-3980-8 in its pull request). No right of the operator changes.

1. **A cluster runner ends itself with a known termination message once it has been idle for its
   pool's `idleAfter`**, and the operator reads it from the pod's status, by the `get` on pods it
   already holds. All a runner gets by saying it is its own suspension.
2. **Idle is read wider than §9, on the safe side:** any open browser counts as running, not only
   one holding an Ask, and so does a link holding a project.
3. **A suspended runner runs again only for the window's steps towards a chat**, relaying a link,
   holding a project or starting a program: the desktop writes the `Idle`
   condition's time in the Claim's annotation `runners.purlis.dev/run-again`. A check, a view, a
   redial or the kill switch never does, as item 5 of the amendment of 2026-10-27 has it. Whoever
   may write it may already set `stopped` back.
4. **A runner's idle time is not part of what is vouched for**, so an operator upgrade or a change
   to `idleAfter` leaves a running runner vouched for, on the time it started with.
