# Amended for #2511, a window takes back its runner chats' sandboxes, 2026-10-18

The amendment for #2444 (item 1) says a confinement is the links' it is named to, and that a
link's names end with it; the amendment for #2407 (item 4) left taking a runner chat's
confinement back after the link between hosts is made again for later. **The person ruled on
2026-10-17** that after a re-link the window may take back the sandboxes of the chats it started
on a runner, its own chats only, never the runner's own window's (ADR 0078, amended 2026-10-18).
Only that ruling is ruled. This note records how a host names them: items 1 to 5 are **decided
in implementation, to be confirmed** (D-2511-1 to D-2511-4). §5 and the amendments above are
left as accepted, and this note adds to them.

1. **A link between hosts names the window it is for** (session protocol 1.22, `take_back`,
   FD-27's row of its own; 1.19 was reserved for it, and released once 1.21 was out first): only `remote-link` sends it, and only the desktop's relay, on each
   link it makes for one window. A link names one window; a second, other name is refused
   `malformed`.
2. **A confinement made or composed for that link is that window's.** The host keeps, with each
   confinement a link between hosts made or had composed for its start, the window that link
   named. A confinement the host's own window made, or one made or composed for a link that
   named no window, is no relayed window's.
3. **`take_back` names the link each chat's confinement of its window, and answers with them.**
   Only a confinement a chat runs in is named: one nothing runs in yet ends with the link that
   made it, as before. From then on the link acts on each and hears its proxy under item 1 of
   the amendment for #2444, as it acted on it before the re-link. This is a window taking its
   chats back by being answered with them, as `programs_of` is on the window's own host
   (item 4 of the amendment for #2444).
4. **A chat is its window's on each of that window's links**: a chat started on one link of a
   window is named to every other link that named the same window, while each lasts.
5. **A chat's ports of the machine** are withdrawn as before when the last link its
   confinement is named to ends (item 5 of the amendment for #2407), and stay withdrawn once
   its window takes it back.
