# Amended 2026-10-17: the window hears a chat a runner composed

Item 1 was **ruled by the operator on 2026-10-17** (item 3 of the amendment of the same day on a
chat confined on a runner): a runner names the confinement it composes for a chat to the window,
in a new minor of the session protocol. Item 2 is **decided in implementation, to be confirmed**
(D-2407-5). ADR 0068, amended for #2407, records how.

1. **A runner names a composed chat's confinement to the link that started the chat**, where the
   start gives its id (session protocol 1.16), so what its proxy refused, carried and asks about
   reaches the window through the desktop's host, and the window's answers and a port withdrawn
   reach it. The desktop's relay sends such a start on for its own window alone. FD-27's
   existing confinement row was amended to hold a start's `confinement`; no row was added.
2. **A runner withdraws a chat's ports of the machine when its peer's link ends**, where that
   link was the last one the chat's confinement was named to: nobody is left there to withdraw
   one. *Decided in implementation, to be confirmed (D-2407-5).*
