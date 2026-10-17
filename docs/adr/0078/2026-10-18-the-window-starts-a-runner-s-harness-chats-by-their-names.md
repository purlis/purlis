# Amended 2026-10-18: the window starts a runner's harness chats by their names

This carries out what the operator ruled on 2026-10-17 (the amendment of that day on a chat
confined on a runner, D-2217-3): a runner takes no harness launch from a relayed link, and
composes its harness chats by their names with the sandbox it compiles for itself (#2446).

1. **The window starts a harness chat on a runner by its names.** Where the window's chats run on
   a runner, a harness chat is started with its profile, its persona, its workspace, the
   conversation it carries on, its first message and its chat and run ids, and nothing the
   window composed: no launch, no sandbox and no confinement of the window's. The runner names
   the confinement it composes to the window's link (session protocol 1.16), so its proxy's
   refusals and asks reach the window and are answered from it, as a local chat's are. A shell
   on a runner is still started from the window's launch. The window sends no launch of a
   harness to a runner, and a start the runner refuses is shown as any start that did not
   start.
2. **A runner chat's run and its sandbox's audit are written on the runner**, in its event log,
   where the chat runs; the window writes neither for it, and does not count it on this machine.
3. **A runner's proxy listens on the runner's ports.** The window holds the confinement of a
   runner chat without judging its ports against, or taking them as, this machine's.
