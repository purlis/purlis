# Amended for #2407, the window hears a chat its host composed, 2026-10-17

The amendment for #2217 (item 6) ruled that a host names the confinement it starts for a chat it
composes by its names to the window, in a new minor of the session protocol, and the amendment
for #2444 (item 4) said how: in that start's answer, to the link that started the chat. This note
records what was built. §5 and the amendments above are left as accepted, and this note adds to
them. Item 1 was **ruled by the operator on 2026-10-17**; items 2 to 6 are **decided in
implementation, to be confirmed** (D-2407-2 to D-2407-6).

1. **Session protocol 1.16.** A start with no `launch` may carry `confinement`, the id the
   confinement the host composes for it is to have. The host then names that confinement to the
   link the start came on, its proxy asks that link about each host nothing lists, and
   `program_started` says where it is (`confined`). A start without it is composed as before: its
   confinement is named to no link, and its proxy asks nobody. 1.15 is taken by #2018. *D-2407-1,
   ruled 2026-10-17 (a protocol addition, in a new minor).*
2. **It is named only where it asks, and only to a link that may confine.** A start's
   `confinement` is in FD-27's confinement row, which was amended to hold it rather than a row
   added: it is refused `not_allowed` from a link that may not confine, and the desktop's relay sends it on for its own `local-ui` client alone. The window
   sends one only where the chat's proxy is to ask, as its own hearing of the chat says (an
   administrator's policy can turn live asks off). So no client of the desktop's host but the
   window is named a composed chat's confinement, on its own host or on a runner, and no ask is
   held where nobody can answer it. *Rejected:* naming every start's composed confinement to its
   link; a separate field for whether it asks. *D-2407-2.*
3. **The window gives the id.** The window hears the id before it sends the start, so nothing the
   proxy does between its start and the answer is missed. A start naming an id the host already
   holds is refused `malformed`, and nothing runs; a start with its own `launch` names its
   confinement in the launch, and a start carrying both is refused `malformed`. *Rejected:* the
   host minting the id and the window learning it from the answer alone. *D-2407-3.*
4. **A relayed link is named only the confinements it made or started.** `programs_of` stays the
   window's on its own host (FD-27), so a runner names its peer no confinement it did not make or
   compose for that peer's start. Taking a runner chat's confinement back after the link between
   hosts is made again is not part of this change. *D-2407-4.*
5. **A chat's ports of the machine go with the last link that could withdraw them.** When the
   last link a chat's confinement is named to ends, and that link is one that cannot take the
   chat back (any but the window on its own host), the host withdraws every port of the machine
   the chat was given, as the window's withdrawal would, and records them as withdrawn in the
   plan it keeps. A port it cannot withdraw ends the confinement. The chat keeps the rest of its
   network. A window on its own host takes its chats back by `programs_of`, and its chats keep
   their ports. A link that ends while the host composes its start leaves the same, once the
   confinement is kept. *Rejected:* ending the chat's whole network; keeping the ports until the chat
   ends. *D-2407-5.*
6. **No changelog entry yet.** The window starts no chat by its names until #2446, so nothing a
   person sees changes here. *D-2407-6.*
