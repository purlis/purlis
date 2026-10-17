# Amended (2026-10-14 and 2026-10-17, the operator's rulings on spec #2008 and #2018): a browser's new sites are judged by its driver

Under the rulings of 2026-10-14 (public sites are readable; an address the agent writes for a
site new to the project asks once; a link the page offered does not; typing or submitting on a
site new to the project asks once; one Ask, one grant system, one Granted list) and the ruling of
2026-10-17 on #2018 (a browser site's Allow is for the browser alone), item 6 of the amendment *a
browser's reach is a lane of its chat's* is built as follows. A site new to the project is a
public site its browser's lane does not list.

1. **The driver tells every request apart; the reach proxy carries a new site only where the
   driver let a request to it go.** A chat's browser holds every http and https request of every
   page, browser-wide, until its judge (`purlis_core::browsers::judge`) has heard what it is: a
   read (a link the page offered, an image, a script, a frame), an address the agent wrote (its
   open or navigate, and where that address redirects while the act lasts), or something sent (a
   form submitted, by GET or POST, a request with a body, or a document the page asked for with a
   query and no word of how it began). The agent's typing, keys and choices are judged as sent to
   the site of the document they go into: the field's own frame, or, where the focus is not
   known, every document the page shows. The reach proxy carries a site new to the project only
   where the judge let a request to it go, or the site is listed.
2. **What asks.** On a site new to the project a read goes; an address the agent wrote, or
   anything sent, is held back and asked about. On the open web nothing asks. While the person
   drives, what they do goes. A browser the person opened with no chat keeps refusing every site
   new to the project at its proxy.
3. **A browser site is a grant of its own, for the browser alone** (ruled 2026-10-17, #2018,
   D-2018-1). A chat's sites wait as one Ask in the registry, every site of the chat on it, with
   Allow for this chat, Always allow in this project on this machine, and Keep blocked. An Allow
   lists the site in the browser's lane only: the judge and the reach proxy read it, and the
   chat's own commands never do, so they still need their own Allow for that host. It is listed
   apart in the Granted list and in the chat's Network view, under "Only its browser can reach"
   (#2364), and removing it ends the browsers it reached; one an
   administrator's policy later holds (a scope left out, a host never allowed) is kept and
   reaches no browser. This supersedes the sentence "nothing is listed for the browser alone"
   in item 1 of the amendment *a browser's reach is a lane of its chat's*. A browser site for
   everyone in the project (committed) is not offered yet: it comes with #2443 (ruled
   2026-10-17). *A host allowed for the chat's commands stays a listed site for
   its browser, as the lane was ruled on 2026-10-14 (D-2018-7): decided in implementation,
   2026-10-17, to be confirmed.*
4. **Widening reaches a running browser at its next act.** Each act carries what the browser
   reaches as it is now (`browse.reach`, session protocol 1.15), so a site allowed while the
   browser runs is reached from its next act on, and the act's answer names each site asked
   about (`browsed.asked`). A narrowed reach still ends the chat's browsers, as before.
   *Decided in implementation, 2026-10-17 (D-2018-2), to be confirmed.*
5. **A chat started without the sandbox is asked nothing.** Until its browser is given what its
   persona would list (#2242), its browser reaches only what it was given, and the chat is told
   so. *Decided in implementation, 2026-10-17, to be confirmed.*
6. **Fails closed.** A browser that was meant to be judged and whose engine will not hold its
   requests does not start. A reach that does not read is refused, and the browser keeps the one
   it had.
