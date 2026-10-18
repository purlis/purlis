# Amended 2026-10-18: the window starts a runner's harness chats by their names

Item 1 carries out what the operator **ruled on 2026-10-17** (the amendment of that day on a chat
confined on a runner, D-2217-3): a runner takes no harness launch from a relayed link, and
composes its harness chats by their names with the sandbox it compiles for itself (#2446). Items
2 to 5 are **decided in implementation, to be confirmed** (D-2446-3, D-2446-7, D-2446-8, and the
ports of a runner's proxy).

1. **The window starts a harness chat on a runner by its names.** Where the window's chats run on
   a runner, a harness chat is started with its profile, its persona, its workspace, the
   conversation it carries on, its first message and its chat and run ids, and nothing the
   window composed: no launch, no sandbox and no confinement of the window's. The runner names
   the confinement it composes to the window's link (session protocol 1.16), so its proxy's
   refusals and asks reach the window and are answered from it, as a local chat's are. A shell
   on a runner is still started from the window's launch.
2. **A runner chat's run and its sandbox's audit are written on the runner**, in its own event
   log, where the chat runs, under the chat and run the window named; the window writes neither
   for it, and does not count it on this machine. *Decided in implementation, to be confirmed
   (D-2446-3).*
3. **The window refuses, in its own words, a start a runner would refuse**: the window's launch
   of a harness, and a person's opt-out of the sandbox, are never sent to a runner. *Decided in
   implementation, to be confirmed (D-2446-7).*
4. **A runner keeps the chat and run ids its desktop relays for the window** (ADR 0066), so what
   it writes of a chat joins the window's own; the desktop's relay sends a start on without the
   ids it names unless its client is the window, and an id the runner already holds is refused.
   This widens #2171's rule, which kept the ids of a `local-ui` start alone. *Decided in
   implementation, to be confirmed (D-2446-8).*
5. **A runner's proxy listens on the runner's ports.** The window holds the confinement of a
   runner chat without judging its ports against, or taking them as, this machine's. *Decided in
   implementation, to be confirmed.*
