# A runner's link down past a blip says why and offers Try now, 2026-11-18

This note bounds *The window is told a runner's link is down* (amendment of 2026-10-18, #2496),
whose item 2 has each of a runner's chats say "Reconnecting…", quietly, until the link is back.
That rule had no end: on 2026-11-17 a Docker pool's chat said "Reconnecting…" over its last
screen for more than five minutes while the core logged, again and again, why the link would
not come up, and the person read it as a chat that would never come back. The operator's ruling
of 2026-11-17: *"we need to fix issues — idiomatically"*: past a short grace period, a status
that names the cause the core already has, when it is tried again, **Try now**, and a way to the
pool's settings, on the window's existing Notice and the existing event, as a remote editor or
an SSH client says a lost connection. What is not quoted is **decided in implementation, to be
confirmed** (D-7134-1 to D-7134-5).

1. **The quiet has a bound: thirty seconds** (`QUIET_FOR_MS`). Until then a link that is down
   is a blip and says only "Reconnecting…" on each chat's pane, as before: most drops come back
   within the first few tries, whose pauses start at a quarter of a second. Past it, the window
   says it once for each pool, in a **Notice** at the window's own level beside a changed key's
   and a runner gone's: never on each chat's pane, which keeps its one quiet state (item 2 of
   the amendment of 2026-10-18). A window drawn long into an outage says it at once.
   *Rejected:* a reason written on each pane, which would say one outage once for every chat.
   *D-7134-1.*
2. **It says why in the host's own words, and that purlis keeps linking.** "Pool docker's runner
   is not linked. Its chats have waited 6 minutes: <why>. purlis keeps linking to it again,
   trying at most 30 seconds apart." The why is what the relaying host last said, never parsed:
   the window decides nothing from it. The pause between tries doubles to at most thirty seconds
   (`hostlink::MOST_PAUSE`), so that is the honest bound on the next try; no time of each try is
   carried, since a try is said only when its reason changes (item 3). *D-7134-2.*
3. **The relaying host says `link_down` again while the link is down, each time why changes**
   (session protocol 1.92). Before, it said it once for each outage, so the window held what
   ended the link ("the link to it ended"), never what the tries since found ("the connector did
   not connect: Docker … didn't answer through SSH"). A try that fails for the reason already
   told says nothing, so a link down for hours is not a word every thirty seconds. A window
   before 1.92 takes the second as the first. The core keeps the last reason for each pool, from
   when the link first went (`runner_links_down`), and drops it once the link is back or a
   changed key or a runner gone says it in a Notice of its own. The window reads it as it is
   drawn and again on each `chats-reconnecting` of its plane, which a new reason sends as it
   dims the chats again. *Rejected:* a reason carried on `chats-reconnecting` itself, which a
   pane drawn during the outage would still have to read, as it reads which chats are dimmed.
   *D-7134-3.*
4. **Try now is `link_now`** (session protocol 1.92), the window's alone and never sent on: the
   relaying host ends its pause between tries and tries at once, or takes links anew where they
   were over. It answers at once; what the try finds is said as ever, `link_back`, or `link_down`
   again with a new reason. A host before 1.92 is sent nothing, and tries after its pause.
   *D-7134-4.*
5. **Check pool opens the pool's own setup** (#4491), where a Docker pool's runner is checked
   step by step; for a cluster pool, which has none, Pools opens with its row. **Dismiss** puts
   the Notice away until the reason changes or the link comes back, so the next outage is said
   again. It goes once the link is back: the window reads which links are down on each
   `chats-reconnecting` of its plane and every 15 seconds while one is, since a host with no
   chat on the runner has none to say the link is back of. *D-7134-5.*
