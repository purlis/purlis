# Amended for #1999, a chat's sandbox and egress proxy live in the session host, 2026-10-15

With the start-time switch on, a chat's confinement, the egress proxy its network leaves
through and the folder and ssh route beside it, is started and held by the session host, so a
chat keeps its network while the window's link is gone. What spec #1974 ruled on 2026-10-14
stands ("everything a chat needs moves at once"; asks reach the window from the host). §5 and
the amendments above are left as accepted, and this note adds to them. Items 1 to 10 are
**decided in implementation, to be confirmed** (D-1999-1 to D-1999-10). With the switch off
nothing changes: the window starts the same plan in its own process.

1. **The window plans, the host starts, with the same code.** The window compiles a chat's
   sandbox as it always has and plans its confinement from it
   (`purlis_core::sandbox::Applied::plan`): every host its proxy carries with its layer, what
   policy pins, the ports withdrawn and given, and what runs beside the proxy. The window's own
   path starts that plan (`Plan::start`); the host reads the plan back exactly and starts it with
   the same function. Two calls, because a chat's line names its proxy's ports: the window asks
   the host to `confine`, and composes the chat's line with the ports the host answers, as it
   does with a proxy of its own. *Rejected:* the host compiling the sandbox again from the same
   inputs (two compiles a moment apart could differ); the line carrying placeholders the host
   fills in. *D-1999-1.*
2. **Session protocol 1.4, a minor.** It adds, each recorded in the protocol's fixture:
   `confine` (the window's id for it, the project, the plan, and whether its proxy asks live),
   answered `confined` with its proxy's two ports and its folders; a start's `launch` naming the
   confinement it runs in; `change_reach` (an Allow at a layer, an Allow forgotten, Keep blocked,
   a port withdrawn), answered `reach_changed` with the hosts to tell the chat to retry;
   `reach_of`, answered `reach` with what its board asks about and allows live;
   `let_go_of_confinement`; and the pushed notice `sandbox`, what a proxy refused, carried and
   asks about. *D-1999-2.*
3. **A chat's confinement is the window's alone.** FD-27's table gains a row, `local-ui` only,
   for the four commands and for hearing `sandbox`: the host pushes `sandbox` only to a link that
   may confine. Every answer to a sandbox ask comes from the window, as it does where the proxy
   runs in the window. *D-1999-3.*
4. **It lives as long as the program in it.** A confinement nothing runs in belongs to the link
   that made it and ends with it, a link told apart by a number no other link has
   (`Peer::link`); one whose link ended while it was being started ends at once. Only a start
   on that link, in the same project, takes it, under the same hold as the start's number, so
   one runs one program at the most; it then ends when that program is closed, when the window
   lets go of it, or when the host ends, and a start that fails ends it. So a window whose link
   is gone leaves every running chat its proxy. *D-1999-4.*
5. **Its proxy holds a connection for an ask only while one can be answered**: a program runs
   in it and a window that may answer is linked. Otherwise a host nothing lists is refused at
   once, as where nobody can be asked in the window (#1666). A host the project's files allow
   already is let on, read in the host by the same code. *D-1999-5.*
6. **What it does reaches the window by the same closures.** The window hands whatever runs its
   chats the closures it would hand a proxy of its own (`purlis_core::sandbox::plan::Hearing`);
   the host's proxy tells the host, the host pushes `sandbox`, and the window tells those
   closures. A Block, a connection kept in the network record and an ask's Notice are raised
   alike. *D-1999-6.*
7. **No port is both a proxy's and a chat's, across the two processes.** The two ports of a
   proxy the host holds are purlis's own in the window too for as long as the window holds the
   chat, and the ports a chat was given are held again where its plan starts; either clash
   refuses the start. A port whose grant is removed while the chat runs is withdrawn from its
   proxy in the host; where the host cannot be told, the window lets go of the chat's
   confinement, so its proxy carries nothing, and says so. *D-1999-7.*
8. **Refusal codes are reused.** A confinement the host does not hold is `no_such_chat`; an id
   it holds already is `malformed`; one its binary could not start, or a start naming one it
   cannot take, is `not_started`. *D-1999-8.*
9. **`purlis serve` confines nothing yet.** The window connects only to the host it started
   (D-1997-5), whose binary confines; `purlis serve` refuses `confine` with a sentence until a
   window links to a host it did not start (#646). *D-1999-9.*
10. **What a chat keeps beside its proxy is written where its holder made the folder.** The
    window writes a chat's own files, such as its copy of a cluster's context, into the folder
    the host made beside the chat's proxy, as it does into its own on the same machine.
    *D-1999-10.*
