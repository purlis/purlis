# Amended 2026-10-21: a failed runner image never holds back a desktop release

**Built by #2880** on item 1 of the ruling of 2026-10-20 for #2749, which made each release
build its runner image. What follows was decided in #2880 and is marked *decided in
implementation, to be confirmed*.

1. **The desktop is released whatever became of its runner image.** In `release.yml`, the
   manifest, provenance and publish jobs run unless the run was cancelled, and each asks for the
   jobs the desktop needs by name, never for the runner image's. Where the image was not built
   or not signed, the manifest names no `runnerImage` and says why in the run's log; the run as a
   whole still shows the image's failure. *Decided in implementation, to be confirmed*
   (D-2880-1).
2. **A release with no runner image moves no pool.** The window notes, at each look, whether its
   version's manifest named a runner image; where it named none, every pool stays on the runner
   it has, and a pool of purlis's own image whose runner is of another version says in the Pools
   group that it is not following purlis and stays on its runner. *Decided in implementation, to
   be confirmed* (D-2880-2).
