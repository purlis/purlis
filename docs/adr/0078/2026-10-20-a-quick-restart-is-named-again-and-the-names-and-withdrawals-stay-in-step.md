# Amended 2026-10-20: a quick restart is named again, and the names and withdrawals stay in step

**Built by #2744** on the amendments of 2026-10-20 for #2623 and #2625. What follows was decided
in #2744 and is marked *decided in implementation, to be confirmed*.

1. **One link of a host holds a window's name at a time, and a quick restart is relayed on a
   second try.** The names the desktop's host's relayed links hold are kept once for the whole
   host, across the relay each link's `relay_to` makes, so item 4 of the amendment for #2623
   holds for every link the host serves: while one link holds a window's name, another that
   names itself so is refused its `relay_to` `not_allowed`, and the host keeps its name for it.
   A window whose `relay_to` is refused so sends it once more on the same link, half a second
   later: the time the host takes to see the end of the window's link from before a restart. A
   second refusal is the link's, and it is not opened. Nothing in the protocol changes. Today
   the window sends no `relay_to` on a project's link yet; the retry is in the one place a
   window link sends it (`HostLink::over_relayed`). *Decided in implementation, to be
   confirmed* (D-2744-1).
2. **A names file past what a private file is read up to is started again.** Where
   `window-names.json` is this user's alone and longer than 64 KiB, it is read as one that does
   not parse: the names are started again and the app's log says so, so later links are still
   named. One another user may read or write, or a link, is still refused. *Decided in
   implementation, to be confirmed* (D-2744-2).
3. **A withdrawal the proxy refuses leaves the plan as the proxy carries it.** A port the window
   withdraws is marked withdrawn in the confinement's plan before its proxy hears it (#2625).
   Where the proxy refuses the withdrawal of a port it was carrying, the mark is taken back: the
   port counts as given, so it is withdrawn as the last link ends like every given port, and
   given back after a re-link. A port the runner had withdrawn for want of a window is not
   carried, and stays marked withdrawn. The window is told the refusal. *Decided in
   implementation, to be confirmed* (D-2744-3).
