# Amended 2026-10-18: a carried-over hold is where the window holds it now

**Built by #2535** on item 2 of the amendment of 2026-10-17 on the desktop's relay. What follows
was decided in #2535 and is marked *decided in implementation, to be confirmed*.

1. **A project is carried over where the window holds it now.** The runner holds a project at
   one place for each link, so a project the window let go of and held again at another path
   while a renewed link was carried over is let go of on the new link where it was carried, and
   held there at the window's new path. *Decided in implementation, to be confirmed*
   (D-2535-1).
2. **A hold is carried once the runner takes it.** A hold the runner refuses in a pass of the
   carry-over is sent again in the next. One the last pass could not carry is no longer kept for
   the window: no later link holds it in the window's name, the desktop's host says so in its
   log, and the runner refuses the window's next command for that project as one it does not
   hold. The window is not told of it yet, and goes on counting the project as held; telling it
   and holding the project again follow in #2645. A let-go the runner refuses keeps the project
   where it was carried, and so it is not held at the new path either. *Decided in
   implementation, to be confirmed* (D-2535-2).
