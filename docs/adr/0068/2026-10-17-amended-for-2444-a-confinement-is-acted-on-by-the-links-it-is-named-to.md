# Amended for #2444, a confinement is acted on by the links it is named to, 2026-10-17

The amendments for #1999 and #2217 say who may confine a chat: the window, on its own host and
as the desktop's host relays it. This note says which confinement each such link acts on and
hears. §5 and the amendments above are left as accepted, and this note adds to them; the
operator's rulings of 2026-10-17 on #2217 stand as ruled. Items 1 to 4 are **decided in
implementation, to be confirmed** (D-2444-1 to D-2444-4).

1. **A confinement is the links' it is named to.** The host names a confinement to a link by
   answering that link with it: `confined` names it to the link that made it, and
   `programs_of` names each confinement it answers to the link that asked, a window taking its
   chats back. A confinement the host started for a chat it composed by its names is named to
   no link when it starts. A link may act on a confinement named to it while nothing runs in
   it and it is that link's own, or, once a program runs in it, while the link holds the
   program's project at the place the program runs. A link's names end with it. So a link that
   may confine a chat acts on the confinements it made or was told, and on no other the host
   holds: not another window's, not another clone's, and not one the host made for a chat it
   composed until the host names it to the link. The same holds on a runner's host for the
   links its peer relays. *D-2444-1.*
2. **Any other confinement is refused as one the host does not hold.** `change_reach`,
   `reach_of` and `let_go_of_confinement` naming a confinement the link may not act on are
   refused `no_such_chat`, and the confinement is unchanged, so a window is never told a chat's
   network ended when it did not. A confinement the host no longer holds is let go of already,
   as before. *Rejected:*
   `not_allowed`, a code of its own for a confinement that is someone else's; answering a let go
   of someone else's as done. *D-2444-2.*
3. **What a proxy did is told to the links that may act on its confinement.** The host pushes
   `sandbox` for a confinement only to a subscribed link that may confine and may act on that
   confinement, and its proxy holds a connection for an ask only while such a link is there to
   answer. *D-2444-3.*
4. **A composed chat's confinement reaches the window by being named to it** (for #2407). When
   the host names the confinement of a chat it composed to the window (ruled 2026-10-17, the
   amendment for #2217 item 6), it names it to the link that started the chat, in that start's
   answer, and a window taking the chat back is named it by `programs_of` as for any other.
   That naming is the link's grant, under item 1: no scope is granted every confinement, and no
   command of its own grants one. *Rejected:* letting every link that may confine act on a
   composed chat's confinement; a new command that asks for one. *D-2444-4.*
