# Amended 2026-10-17: the desktop's relay keeps its link to a runner

**Built by #2215** on item 5 of the amendment of 2026-10-14 (the operator's ruling of that day:
*"a link-level liveness ping and reconnect, and reconnecting ahead of the node's stream
limits"*). §2 and §4 are left as accepted, and this note says what item 5 means for the window's
links through the desktop's host. What is not quoted from the ruling was decided in #2215 and is
marked *decided, to be confirmed*.

1. **The desktop's host keeps one link to the runner for each window link it relays**, through
   the same kept link as any other (item 5): linked again on its own after a drop, with the same
   pauses, renewed ahead of a stream's limit, and only to the host whose key was pinned at
   pairing. A host at the far end with another key is refused on every try. *Decided, to be
   confirmed.*
2. **Over each new link it carries over what the window's link stands on, and nothing else**:
   the projects the window holds there, its subscription from the window's cursor (the last
   event it handed the window, so none comes twice), and the window's views, each attached
   again with a snapshot under the number the window knows it by, at its next epoch, so the
   window draws it as a view that came back (ADR 0068 §4). *Decided, to be confirmed.*
3. **A renewal leaves nothing unanswered.** The link a renewal replaces carries the window's
   commands while the new one is carried over, and ends once the relay has moved to the new one
   and nothing sent on it waits for an answer, or after the ping's deadline at the latest.
   *Decided, to be confirmed.*
4. **No command is sent twice.** A command a dropped link ended under is refused as unanswered;
   while no link is up, every command the window sends is refused in words saying the runner is
   not linked, and is not kept for later. The window decides what to send again. A view the
   runner will not attach again has ended for good, and the window is told so (`view_ended`).
   *Decided, to be confirmed.*
5. **What the runner named to a link ends with that link** (ADR 0068, amended for #2444): a
   new link is named nothing again by being carried over. So after a re-link, a renewal
   included, the confinement of a chat the window started on the runner is not the new link's
   to ask, change or let go of, and the runner refuses it as one it does not hold; once no link
   it was named to is left, the runner withdraws the chat's ports on its machine (#2482). The window
   takes its own runner chats' sandboxes back after a re-link in #2511, as ruled. Until then no
   window links to a runner through a kept relay.
6. **The window is told when a runner's link is down and back** in #2496, as ruled: a pushed
   notice, and "reconnecting" over each view's last frame.
