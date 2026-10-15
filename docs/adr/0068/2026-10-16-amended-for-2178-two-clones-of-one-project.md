# Amended for #2178, two clones of one project in one window, 2026-10-16

With chats running in the session host (spec #1974, ruled 2026-10-14), one window may hold two
clones of the same project, the same `[project] id` at two folders, and start chats in each. A
project is still named on the wire by its stable id alone, and a path is never a key (the
amendment by FD-26, item 5). §1, §4, §5 and the amendments before this one are left as accepted,
and this note adds to them. Both items are **decided in implementation, to be confirmed**
(D-2178-1 and D-2178-2). No session protocol word, key or version changes.

1. **The host holds a project at a place, and a link holds each project at one place.** What
   the host holds is a project's id at a folder on its machine. A link holds a project at one
   place until it lets go; another link may hold the same project at another place; no hold
   moves where another link holds it, and a link holding a project at a second place is refused
   `not_allowed`. Each command that names a project means the place its link holds it at, or,
   for a link that holds it nowhere, the one place it is held; a project held at two places,
   neither of them the link's, is refused `no_such_project`. Each place has its own hook
   channel, and its lines are pushed only to the links that hold it there. The window holds each
   project root on a `local-ui` link of its own, which that root's chats and browsers share; its
   first link carries the kill switch. This replaces D-1998-5's "a project held at one path is
   not held again at another". *D-2178-1, to be confirmed.*
2. **Numbers are dealt per place.** Each clone keeps its own chats' record and per-chat files,
   so a number names a chat of that clone, as it does when chats run in the window's own
   process. *D-2178-2, to be confirmed.*
