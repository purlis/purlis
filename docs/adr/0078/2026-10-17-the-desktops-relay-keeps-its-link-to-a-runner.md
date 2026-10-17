# Amended 2026-10-17: the desktop's relay keeps its link to a runner

**Built by #2215** on item 5 of the amendment of 2026-10-14 (the operator's ruling of that day:
*"a link-level liveness ping and reconnect, and reconnecting ahead of the node's stream
limits"*). §2 and §4 are left as accepted, and this note says what item 5 means for the window's
links through the desktop's host. What is not quoted from the ruling was decided in #2215 and is
marked *decided, to be confirmed*.

1. **The desktop's host keeps one link to the runner for each window link it relays**, through
   the same kept link as any other (item 5): linked again on its own after a drop, with the same
   pauses, renewed ahead of a stream's limit, and only to the host whose key was pinned at
   pairing. A host at the far end with another key is refused on every try.
2. **Over each new link it carries over what the window's link stands on, and nothing else**:
   the projects the window holds there, its subscription from the window's cursor (the last
   event it handed the window, so none comes twice), and the window's views, each attached
   again with a snapshot under the number the window knows it by, at its next epoch, so the
   window draws it as a view that came back (ADR 0068 §4). The window's commands wait while that
   is done. *Decided, to be confirmed.*
3. **No command is sent twice.** A command the link ended under is refused as unanswered; while
   no link is up, every command the window sends is refused in words saying no link is up, and
   is not kept for later. The window decides what to send again. *Decided, to be confirmed.*
4. **A view the runner will not attach again has ended for good**, and the window is told so
   (`view_ended`). What a link confined that no chat runs in yet ends with that link, on the
   runner as on the desktop (#1999). *Decided, to be confirmed.*
