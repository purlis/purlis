# Amended 2026-10-18: the window is told a runner's link is down and back

**Built by #2496** on item 6 of the amendment of 2026-10-17 on the desktop's relay, as **ruled by
the operator on 2026-10-17**: *"The relay pushes the link going down and coming back to the
window in a new session-protocol minor (additive). The window shows that runner's chats as
reconnecting, quietly, and clears it when the link is back."* What is not quoted from the ruling
was decided in #2496 and is marked *decided in implementation, to be confirmed*.

1. **The relay pushes `link_down`, with why, and `link_back`** (session protocol 1.17), to
   every subscriber of the client link whose link to the runner went down or came back: once
   each time no link is up, and once when one is up again and carries what the client's link
   stands on. A link renewed ahead of its limit is neither. A relay that dials a link for each
   command after a drop (`Relay::new`) says both as the kept one does. They are the relaying
   host's own words: the same words pushed by the runner are never handed on, and neither holds
   up linking again for a client that has stopped reading: `link_down` is dropped where it does
   not fit, and `link_back`, one word for each outage, waits on a task of its own. *Decided in
   implementation, to be confirmed* (D-2496-1, D-2496-2).
2. **The window shows each of that runner's chats as reconnecting**: its last screen dimmed under
   "Reconnecting…", one quiet state over the terminal, never a Notice. The chats named are every
   chat the project's link to that host runs. A chat is reconnected when the link is back, when
   a view of it draws a snapshot, which comes only over a link that is up (a view drawn again,
   item 2 of the amendment of 2026-10-17, or one opened since), or when the window can no longer
   tell, having fallen behind its host or lost its link to it; the window keeps which chats it
   dimmed, so each is cleared, and a pane drawn during an outage starts dimmed. A runner's
   browser views do not show it yet: they ride another link, and follow in #2542. *Decided in implementation, to be confirmed* (D-2496-3, D-2496-7, D-2496-8).
