# Amended for #2445, a relayed plan fits the runner, 2026-10-17

The amendment for #2217 lets a runner's host start a chat's confinement as the desktop's host
relays the window's plan. This note says what the runner holds that plan to. The operator's
rulings of 2026-10-17 on #2217 stand as ruled, and the amendments for #1999, #2217 and #2444
are left as accepted. Items 1 to 4 are **decided in implementation, to be confirmed**
(D-2445-1 to D-2445-4).

1. **A plan made on another machine fits the host's machine, or nothing starts.** A `confine`
   from any link but the host's own window (`local-ui`) is held to the project as the host
   holds it, before anything starts:
   - a Claude Code chat's ssh route goes in the host's own folder for the project's routes;
   - every service of this machine the plan gives the chat is one the host's own sandbox for
     the project gives a chat it starts: yours for the project on this machine, at the scope
     policy leaves open, with no grant of another machine's;
   - an address of this machine is listed only as its loopback, `127.0.0.1` or `::1`, on such
     a port.

   The hosts elsewhere the plan lists, its never-hosts and its withdrawn ports are the window's,
   as planned. *D-2445-1.*
2. **One that does not fit is refused `not_allowed`**, with a sentence naming what does not
   fit. `not_allowed` already says that what a link asks is not the host's to give.
   *Rejected:* `not_started`, which reads as a start that failed; a refusal code of its own,
   which is a new word on the wire. *D-2445-2.*
3. **The host keeps the plan as it is: it does not map it onto its own folders and ports.** The
   host already chooses every port it listens on for a confinement (its proxy's, answered in
   `confined`) and every folder it makes beside it. A given port names a service that already
   listens on the machine, so there is nothing to map it to. *Rejected:* rewriting the plan's
   ports and routes folder to the host's, because the window would then show a chat's reach
   that is not the reach it has. *D-2445-3.*
4. **The window's plan on its own host, and a plan the host makes itself, are started as
   planned.** On its own machine the window plans from the same project files, with the grants
   it keeps for each chat. A confinement the host plans for a chat it composes by its names is
   compiled there already (the amendment for #2171). *D-2445-4.*

The binary checks the plan through the host's `Confine` seam (`fits`), which every binary
implements. The app's and `purlis serve`'s implementations both use the core's
`sandbox::hosted::fits_here`.
