# Amended for #2525, a desktop unpairs the runner it removes, 2026-10-18

ADR 0078 §3 says that removing a runner removes the desktop's key from the runner's peers when
the link is up. This note says how the session protocol carries that. The amendments above are
left as accepted, and this note adds to them. The minor was assigned on 2026-10-18; the rest is
*decided in implementation, to be confirmed* (D-2525-3).

1. **Session protocol 1.25, a minor.** `unpair` takes no field and is answered `done`. It has a
   row of its own in FD-27's table (`Power::Unpair`), granted to `remote-link` alone. Only a
   runner's host carries it out, on its link between hosts from the desktop it is paired with:
   it takes the desktop the link's handshake proved out of its peers, so no later link from it
   is admitted, and it ends that link once it has answered. Every other host refuses it
   `not_allowed`, and a relaying host never sends it on. A host before 1.25 refuses it
   `unknown_command`. Recorded in the protocol's fixture.
