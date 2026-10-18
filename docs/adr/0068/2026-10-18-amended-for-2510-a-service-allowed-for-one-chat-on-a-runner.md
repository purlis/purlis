# Amended for #2510, a service allowed for one chat, on a runner, 2026-10-18

The amendment for #2445 holds a plan made on another machine to the runner that starts it, and
ended with a line until #2510. This note replaces that line; everything else in it stands as
accepted, and the operator's rulings of 2026-10-17 and 2026-10-18 on #2445 stand as ruled.

The operator ruled for this note, 2026-10-18: **the person is asked, per chat**. Item 3 is that
ruling; it was D-2510-3, an open choice until he ruled, so no decision of that number is left
to confirm. Items 1, 2, 4 and 5 are **decided in implementation, to be confirmed** (D-2510-1,
D-2510-2, D-2510-4, D-2510-5).

1. **A service the person allowed for one chat alone never keeps a runner chat from
   starting.** The window plans such a service as it always has: its port among those the plan
   gives, and its loopback entries at the chat's layer and at no other. Where the runner's own
   sandbox for the project does not give that port, the runner starts the plan without it (its
   port and those entries) instead of refusing the plan. Nothing else in the plan changes, and
   every other check of the amendment for #2445 still applies. A port given at any other
   layer, or with no entry of the chat's, is held to those checks as before. A host older than
   session protocol 1.20 still refuses such a plan, as it did. *Rejected:* refusing the plan,
   which leaves a chat the person allowed more to with nothing at all. *D-2510-1.*
2. **The runner says what it left out, and the window says so.** Session protocol 1.20, a
   minor: `confined` may name those ports (`left_out`) and the runner's own name, as its system
   names it (`host_name`). The chat's tab says the runner does not give the chat those
   services, then whether the chat is asked about them there or cannot reach them, by whether
   its proxy asks live. *Rejected:* rewriting the plan with nothing said, which shows the
   person a reach the chat does not have (D-2445-4). *D-2510-2.*
3. **The person allows a service left out for the chat on the runner, through the sandbox's
   Ask** (ruled 2026-10-18). The runner keeps the ports it left out, and only those, as ones
   that confinement's proxy may ask about. A connection the chat makes to one of them, on the
   runner's loopback by any name the proxy carries one by, is held while the person is asked,
   as a host nothing lists is. The window asks it as a service on that runner, by its port and
   the runner's name, never as one on the person's own machine. The person's answer reaches
   the runner as the existing change of reach, about the port at the loopback address. The
   runner takes it only for a port it left out of that confinement's plan, and refuses any
   other port of its own in a sentence. Allowed, it is carried for that chat alone, for as
   long as its confinement runs, and is never kept on the runner; kept blocked, the held
   connection is refused at once, and so is every later one until an Allow; an Allow removed
   reaches it no more. A port withdrawn from the chat is asked about no more. A proxy that asks
   nobody refuses such a connection at once, as before.
4. **The ask is held on the runner's own board, and is no grant there.** The board keeps the
   ports allowed this way apart from the hosts allowed live, so they are never told back to
   the window as a grant to carry into the chat's next start. *Rejected:* listing them as hosts
   allowed live, which the next start would plan as a service the runner does not give.
   *D-2510-4.*
5. **Such an ask is answered once or for this chat, and writes no grant on the window's
   machine.** Its Notice and its Ask offer Allow once and Allow for this chat, and the window
   takes no other scope for it. The Allow is audited and reaches only the chat whose
   confinement asked, on its runner. Nothing is kept on the person's machine, so the chat's
   next start plans the service as before, at the chat's layer, and runs on the runner again.
   *Rejected:* a wider scope, which would write a grant for every chat of the project on the
   person's own machine, plan that port at that scope, and so have the runner refuse the chat's
   next start. *D-2510-5.*
