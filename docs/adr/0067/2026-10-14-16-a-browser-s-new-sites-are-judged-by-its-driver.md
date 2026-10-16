# Amended (2026-10-14, the operator's rulings on spec #2008, built for #2018): a browser's new sites are judged by its driver

Under the rulings of 2026-10-14 (public sites are readable; an address the agent writes for a
site new to the project asks once; a link the page offered does not; typing or submitting on a
site new to the project asks once; the project's "open web" turns the ask off; one Ask, one grant
system, one Granted list), item 6 of the amendment *a browser's reach is a lane of its chat's*
is built as follows. A site new to the project is a public site its chat's reach does not list.

1. **The driver tells every request apart; the reach proxy carries only what was judged.** A
   chat's browser holds every request of every page, browser-wide, until its judge
   (`purlis_core::browsers::sites`) has heard what it is: a read (a link the page offered, an
   image, a script, a frame), an address the agent wrote (its open or navigate, and where that
   address redirects while the act lasts), or something sent (a form submitted, by GET or POST,
   or a request with a body). The agent's typing, keys and choices on a page are judged as
   something sent to the page's site. The reach proxy then carries a site new to the project
   only where a request to it was judged to go, so nothing that does not pass the driver reaches
   one.
2. **What asks.** On a site new to the project a read goes; an address the agent wrote, or
   anything sent, is held back and asked about. On the open web nothing asks. While the person
   drives, what they do goes. A browser the person opened with no chat keeps refusing every site
   new to the project at its proxy.
3. **The Ask is the chat's host Ask.** The act's answer names each site asked about
   (`browsed.asked`, session protocol 1.15) and tells the chat it was asked and not to retry
   until told. The window asks about each as a host its chat's sandbox refused, so every site
   and host one chat waits on is one Notice with one Allow, answered from the asks registry, and
   allowed at the scopes a host is. An Allow lists the site in the chat's reach and shows in the
   one Granted list. *Decided in implementation, 2026-10-17 (D-2018-1).*
4. **Widening reaches a running browser at its next act.** Each act carries the chat's reach as
   it is now (`browse.reach`, 1.15), so a site allowed while the browser runs is reached from its
   next act on. A narrowed reach still ends the chat's browsers, as before. *Decided in
   implementation, 2026-10-17 (D-2018-2).*
5. **Fails closed.** A browser that was meant to be judged and whose engine will not hold its
   requests does not start. A reach that does not read is refused, and the browser keeps the one
   it had.
