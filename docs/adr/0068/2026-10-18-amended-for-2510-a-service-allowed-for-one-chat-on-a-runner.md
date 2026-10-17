# Amended for #2510, a service allowed for one chat, on a runner, 2026-10-18

The amendment for #2445 holds a plan made on another machine to the runner that starts it, and
ended with a line until #2510. This note replaces that line; everything else in it stands as
accepted, and the operator's rulings of 2026-10-17 and 2026-10-18 stand as ruled. Each item is
**decided in implementation, to be confirmed** (D-2510-1 to D-2510-3).

1. **A service the person allowed for one chat alone never keeps a runner chat from
   starting.** The window plans such a service as it always has: its port among those the plan
   gives, and its loopback entries at the chat's layer and at no other. Where the runner's own
   sandbox for the project does not give that port, the runner starts the plan without it (its
   port and those entries) instead of refusing the plan. Nothing else in the plan changes, and
   every other check of the amendment for #2445 still applies. A port given at any other
   layer, or with no entry of the chat's, is held to those checks as before. The chat does not
   reach the service on the runner. *Rejected:* refusing the plan, which leaves a chat the
   person allowed more to with nothing at all. *D-2510-1.*
2. **The runner says what it left out, and the window says so in a sentence.** Session
   protocol 1.17, a minor: `confined` may name those ports (`left_out`). The window says on the
   chat's tab that the runner does not give it those services, which were allowed for this chat
   alone on this machine. A host before 1.17 never leaves one out. *Rejected:* rewriting the
   plan with nothing said, which shows the person a reach the chat does not have (D-2445-4).
   *D-2510-2.*
3. **Allowing such a service for the chat on the runner itself waits for the operator.** It is
   a change to what a grant may give a chat planned on another machine, so it is not part of
   this note. Until then the runner gives a relayed chat only what its own sandbox for the
   project gives. *D-2510-3.*
