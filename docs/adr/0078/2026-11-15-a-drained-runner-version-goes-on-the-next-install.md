# Amended 2026-11-15: a drained runner version goes on the next install

**Built by #6688**, on §5 as V22a and the amendment of this day (*a runner's install checks and
places in one command*) settled it, and on ADR 0068 §8's side by side. RR-14 left an old version's
folders on the runner for ever; this removes them once its host has drained. What follows was
decided in implementation and is marked *decided in implementation, to be confirmed*.

1. **Which versions may go: every one older than the one before the running one.** The running
   version (the one the desktop installs) and the one before it always stay, so a desktop one
   version behind still finds its binary there, and so do a newer one, which is not this
   desktop's to remove, and a folder whose name is not read as a version. Versions are ordered as
   semver orders them (`1.10.0` after `1.9.0`, a dev build before its release)
   (`runnerinstall::prune::may_go`). *Decided in implementation, to be confirmed* (D-6688-1).
2. **A version goes only once its host has drained, and the runner decides it, not a shell.**
   The running version's own binary decides on the runner (`purlis bridge --prune`, run as
   `exec sh -c '<config>/server/<ver>/purlis bridge --prune'`, both values held to
   `runnerinstall`'s plain-value checks). For each version that may go, it takes that version's
   host lock (the `flock` a host holds while it serves) and keeps it while it looks and removes,
   so no host of it starts in between. The version stays when the lock is held, when something
   answers on its socket (a host whose lock file the system cleaned away), or when its
   `hosts.jsonl` names programs (a host killed outright left chats that may still run; a host
   that ends as it should empties it). Doubt keeps it: a lock it cannot open, or a record it
   cannot read, counts as live. What the runner prints (`removed=<ver>`) is what the desktop
   says it removed, never what it decides by. *Decided in implementation, to be confirmed*
   (D-6688-2).
3. **The binary's folder goes first, then the host's, then what the host left in the runtime
   folder**: `<config>/server/<ver>/`, so nothing can start it again, then
   `<config>/purlisd/server/<ver>/`, then its socket, link socket and lock file. A folder that is
   a link is removed as a link. *Decided in implementation, to be confirmed* (D-6688-3).
4. **It runs on every install, a version already there included, and never fails one.** Each
   desktop connection that checks its version is in place prunes, which is *the next install or
   probe* of #6688. A prune that fails leaves the old versions for the next. An SSH runner's host
   does not stop by itself when its last chat ends (only a cluster runner's suspends when idle),
   so an old version goes once its host is stopped: by the person, by a restart of the machine,
   or by RR-15's supervisor. *Decided in implementation, to be confirmed* (D-6688-4).
