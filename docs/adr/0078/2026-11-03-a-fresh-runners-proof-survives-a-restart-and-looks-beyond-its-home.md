# Amended 2026-11-03: a fresh runner's proof survives a host restart and looks beyond its home

**Built by #4933**, following #4781 (a fresh runner's proof looks through its home outside the
project) and D-4781-3 (at the sandbox's floor a runner's chat may write anywhere in its home, so
the proof covers the home, not only the project). What follows is *decided in implementation,
to be confirmed* (the D-lines of the pull request that built it).

1. **The cutoff is kept beside the projects.** The time a runner's host first starts a program
   is the status change time of an empty file, `<data>/projects/.first-program`, made, never
   through a link, before that program, and never again while it is there. It is on the home's
   volume, so a host started again in the same pod keeps it. The host also holds its own first
   start in memory and takes the earlier of the two.
2. **No lasting cutoff, no program.** A host that can't make the mark, or finds anything but a
   regular file in its place, refuses the start as not started. Something else read in its
   place is said, which keeps the runner.
3. **The mark is as writable as the home it guards.** A task that moves or removes it could as
   well delete its own work: the proof keeps a task's work from being lost, not from being
   hidden by the task.
4. **On Linux, the rest of the runner's file system is looked through too**, from `/`, across
   mounts and never through a link, for files and links the runner's user owns changed since
   the cutoff. Left out: the kernel's views of itself (`/proc`, `/sys`, `/dev`), what is
   temporary (`/run`, `/tmp`, `/var/tmp`), the home (looked through by its own rules), purlis's
   data home and projects, and the hidden entries of a person's home (`/root`, `/home/<name>`).
   A folder that can't be read is said, unless it is another's, unchanged since, that the user
   can't write in either. A folder deeper than 128 is said, not looked into.
5. **Elsewhere, the home only.** Fresh runners are cluster pods, on Linux; on macOS `/` reaches
   other volumes, and the home again by another path.
6. **What can't be looked through keeps the runner**, as everything the proof can't read does:
   an image with more folders than the walk's bound, or a folder its user may write in but not
   read, keeps its fresh runners until the person discards them.
